"""Local meeting tools. JSON-lines progress on stdout; no audio leaves this machine."""
import argparse
import json
import math
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tarfile
import tempfile
import urllib.request
import wave

SEG_URL = "https://github.com/k2-fsa/sherpa-onnx/releases/download/speaker-segmentation-models/sherpa-onnx-pyannote-segmentation-3-0.tar.bz2"
EMB_URL = "https://github.com/k2-fsa/sherpa-onnx/releases/download/speaker-recongition-models/wespeaker_en_voxceleb_resnet34.onnx"
CALIBRATION_URL = "https://github.com/k2-fsa/sherpa-onnx/releases/download/speaker-recongition-models/3dspeaker_speech_campplus_sv_zh_en_16k-common_advanced.onnx"
FLAGS = {"creationflags": subprocess.CREATE_NO_WINDOW} if os.name == "nt" else {}


def progress(stage, percentage, message):
    print(json.dumps(dict(stage=stage, percentage=percentage, message=message), ensure_ascii=False), flush=True)


def atomic_json(path, value):
    path = Path(path)
    temp = path.with_suffix(".tmp")
    temp.write_text(json.dumps(value, ensure_ascii=False), encoding="utf-8")
    temp.replace(path)


def run(args):
    # A log file avoids pipe deadlocks and holds only engine diagnostics, never credentials.
    with tempfile.TemporaryFile() as log:
        result = subprocess.run(args, stdout=log, stderr=log, **FLAGS)
        if result.returncode:
            log.seek(0)
            raise RuntimeError(log.read().decode("utf-8", errors="replace")[-2500:])


def download(url, destination, stage):
    temp = destination.with_suffix(".download")
    with urllib.request.urlopen(url, timeout=60) as response, temp.open("wb") as out:
        total = int(response.headers.get("Content-Length", 0))
        received = 0
        while True:
            block = response.read(1024 * 1024)
            if not block:
                break
            out.write(block)
            received += len(block)
            progress("setup", min(99, received / total * 100) if total else 0, stage)
    temp.replace(destination)


def setup(root):
    root.mkdir(parents=True, exist_ok=True)
    python = root / "runtime" / ("Scripts/python.exe" if os.name == "nt" else "bin/python")
    progress("setup", 0, "Подготовка локального движка голосов…")
    if not python.exists():
        run([sys.executable, "-m", "venv", str(root / "runtime")])
    run([str(python), "-m", "pip", "install", "--disable-pip-version-check", "sherpa-onnx==1.13.8", "numpy==2.2.6"])
    segmentation = root / "segmentation.onnx"
    if not segmentation.exists():
        archive = root / "segmentation.tar.bz2"
        download(SEG_URL, archive, "Скачивание модели разделения речи…")
        with tarfile.open(archive) as tar:
            member = next(m for m in tar.getmembers() if m.name.endswith("/model.onnx") and m.isfile())
            with tar.extractfile(member) as source, segmentation.with_suffix(".download").open("wb") as out:
                shutil.copyfileobj(source, out)
        segmentation.with_suffix(".download").replace(segmentation)
        archive.unlink()
    if not (root / "embedding.onnx").exists():
        download(EMB_URL, root / "embedding.onnx", "Скачивание модели отпечатков голосов…")
    run([str(python), "-c", "import sherpa_onnx, numpy"])
    (root / "ready").write_text("sherpa-onnx 1.13.8", encoding="utf-8")
    progress("setup", 100, "Движок голосов готов")


def import_audio(args):
    folder = Path(args.output).parent
    folder.mkdir(parents=True, exist_ok=True)
    destination = folder / "audio.wav"
    progress("import", 5, "Подготовка аудио…")
    run([args.ffmpeg, "-nostdin", "-v", "error", "-y", "-i", args.source, "-vn", "-ac", "1", "-ar", "16000", "-c:a", "pcm_s16le", str(destination)])
    with wave.open(str(destination)) as audio:
        duration = audio.getnframes() / audio.getframerate()
    if duration <= 0:
        raise RuntimeError("В записи нет аудио")
    atomic_json(args.output, dict(audioPath=str(destination), durationSeconds=duration))
    progress("import", 100, "Аудио готово")


def analyze(args):
    import numpy as np
    import sherpa_onnx as sherpa
    root = Path(args.root)
    config = sherpa.OfflineSpeakerDiarizationConfig(
        segmentation=sherpa.OfflineSpeakerSegmentationModelConfig(
            pyannote=sherpa.OfflineSpeakerSegmentationPyannoteModelConfig(model=str(root / "segmentation.onnx")), num_threads=2),
        embedding=sherpa.SpeakerEmbeddingExtractorConfig(model=str(root / "embedding.onnx"), num_threads=2),
        clustering=sherpa.FastClusteringConfig(num_clusters=args.speakers or -1, threshold=0.7),
        min_duration_on=0.5, min_duration_off=0.5)
    if not config.validate():
        raise RuntimeError("Модели голосов повреждены. Повторите установку движка.")
    with wave.open(args.audio) as audio:
        if audio.getframerate() != 16000 or audio.getnchannels() != 1 or audio.getsampwidth() != 2:
            raise RuntimeError("Ожидалось моно аудио PCM16 16 кГц")
        samples = np.frombuffer(audio.readframes(audio.getnframes()), dtype="<i2").astype(np.float32) / 32768
    engine = sherpa.OfflineSpeakerDiarization(config)
    last_percentage = [-1]
    def update(done, total):
        percentage = int(100 * done / max(total, 1))
        if percentage != last_percentage[0]:
            progress("analyze", percentage, "Различаем голоса участников…")
            last_percentage[0] = percentage
        return 0
    turns = [dict(start=float(r.start), end=float(r.end), speakerId=f"speaker-{r.speaker + 1}")
             for r in engine.process(samples, callback=update).sort_by_start_time()]
    if not turns:
        raise RuntimeError("Речь не найдена. Проверьте запись.")
    atomic_json(args.output, turns)


def assign_speaker(start, end, turns):
    scores = {}
    for turn in turns:
        overlap = max(0, min(end, turn["end"]) - max(start, turn["start"]))
        if overlap:
            key = turn["speakerId"]
            scores[key] = scores.get(key, 0) + overlap
    ordered = sorted(scores, key=scores.get, reverse=True)
    if not ordered:
        # Word timestamps may extend just past a segmentation boundary. Nearby
        # labels remain provisional and explicitly marked for human review.
        nearest = min(turns, key=lambda t: max(t["start"] - end, start - t["end"], 0), default=None)
        if nearest and max(nearest["start"] - end, start - nearest["end"], 0) <= 0.3:
            return nearest["speakerId"], True
        return None, True
    # Uncovered speech and overlapping voices remain visible for human review.
    uncertain = ordered[0] is None or scores[ordered[0]] < (end - start) * 0.5 or (len(ordered) > 1 and scores[ordered[1]] > scores[ordered[0]] * 0.35)
    return ordered[0], uncertain


def align_segments(result, offset, turns):
    rows = []
    for segment in result.get("transcription", []):
        # Full JSON includes timed tokens; assigning each token avoids one speaker
        # label swallowing an entire Whisper sentence across a change of voice.
        tokens = segment.get("tokens", [])
        timed = [t for t in tokens if t.get("offsets", {}).get("from", -1) >= 0
                 and t.get("offsets", {}).get("to", -1) > t["offsets"]["from"]
                 and not t.get("text", "").startswith("[_")]
        # Use tokens only when they cover all speech text (never drop untimed words).
        speech_tokens = [t for t in tokens if not t.get("text", "").startswith("[_")]
        units = timed if timed and len(timed) == len(speech_tokens) else [segment]
        for unit in units:
            times = unit.get("offsets", {})
            start = offset + times.get("from", 0) / 1000
            end = offset + times.get("to", 0) / 1000
            text = unit.get("text", "")
            if not text.strip() or end <= start:
                continue
            speaker, review = assign_speaker(start, end, turns)
            if rows and rows[-1]["speakerId"] == speaker and start - rows[-1]["end"] < 1.2 and end - rows[-1]["start"] < 20:
                rows[-1]["end"] = end
                rows[-1]["text"] += text
                rows[-1]["needsReview"] |= review
            else:
                rows.append(dict(start=start, end=end, text=text, speakerId=speaker, needsReview=review))
    for row in rows:
        row["text"] = row["text"].strip()
    return rows


def classify_similarity(scores, ids, minimum=0.5, margin=0.12):
    """Cosine similarities are decision scores, not calibrated probabilities."""
    order = sorted(range(len(scores)), key=lambda i: float(scores[i]), reverse=True)
    top = order[0]
    gap = float(scores[top]) - (float(scores[order[1]]) if len(order) > 1 else 0)
    if float(scores[top]) < minimum or gap < margin:
        return None
    return ids[top]


def relabel_segments(segments, turns):
    output = []
    for segment in segments:
        row = dict(segment)
        if row.get("speakerLocked"):
            output.append(row)
            continue
        duration = row["end"] - row["start"]
        totals = {}
        for turn in turns:
            overlap = max(0, min(row["end"], turn["end"]) - max(row["start"], turn["start"]))
            if overlap and turn["speakerId"] is not None:
                key = turn["speakerId"]
                totals[key] = totals.get(key, 0) + overlap
        ordered = sorted(totals, key=totals.get, reverse=True)
        best = ordered[0] if ordered else None
        # Mixed sentences need word-level splitting or human review; never
        # silently assign a whole sentence containing two distinct speakers.
        accepted = best is not None and totals[best] >= duration * 0.7 and (len(ordered) < 2 or totals[ordered[1]] < duration * 0.2)
        row["speakerId"] = best if accepted else None
        row["needsReview"] = not accepted
        output.append(row)
    return output


def calibrate(args):
    import numpy as np
    import sherpa_onnx as sherpa
    meeting = json.loads(Path(args.meeting).read_text(encoding="utf-8"))
    references = json.loads(Path(args.references).read_text(encoding="utf-8"))
    root = Path(args.root)
    model = root / "campplus.onnx"
    if not model.is_file():
        download(CALIBRATION_URL, model, "Скачивание модели калибровки голосов…")
    extractor = sherpa.SpeakerEmbeddingExtractor(sherpa.SpeakerEmbeddingExtractorConfig(model=str(model), num_threads=2))
    with wave.open(meeting["audioPath"]) as audio:
        samples = np.frombuffer(audio.readframes(audio.getnframes()), dtype="<i2").astype(np.float32) / 32768
    duration = len(samples) / 16000

    def embedding(start, end):
        clip = np.ascontiguousarray(samples[max(0, int(start * 16000)):min(len(samples), int(end * 16000))])
        if len(clip) < 16000 or float(np.sqrt(np.mean(clip ** 2))) < 0.0002:
            return None
        stream = extractor.create_stream()
        stream.accept_waveform(sample_rate=16000, waveform=clip)
        stream.input_finished()
        if not extractor.is_ready(stream):
            return None
        vector = np.asarray(extractor.compute(stream), dtype=np.float32)
        norm = float(np.linalg.norm(vector))
        return vector / norm if norm > 1e-9 else None

    progress("calibrate", 0, "Проверяем подтверждённые образцы…")
    ids = list(dict.fromkeys(r["speakerId"] for r in references))
    names = {r["speakerId"]: r["name"] for r in references}
    vectors = []
    for speaker in ids:
        chunks = []
        for ref in [r for r in references if r["speakerId"] == speaker]:
            for start in np.arange(ref["start"] + 0.5, ref["end"] - 3, 3):
                vector = embedding(float(start), float(start + 3))
                if vector is not None:
                    chunks.append(vector)
        if len(chunks) < 2:
            raise RuntimeError(f"Для {names[speaker]} нужно не менее 6–10 секунд чистой речи.")
        vectors.append(np.stack(chunks))
    centers = np.stack([v.mean(axis=0) / np.linalg.norm(v.mean(axis=0)) for v in vectors])
    validation = []
    for index, chunks in enumerate(vectors):
        correct = 0
        for j, vector in enumerate(chunks):
            other = np.delete(chunks, j, axis=0).mean(axis=0)
            candidates = centers.copy()
            candidates[index] = other / np.linalg.norm(other)
            correct += int(int(np.argmax(vector @ candidates.T)) == index)
        validation.append(dict(speakerId=ids[index], name=names[ids[index]], windows=len(chunks), correct=correct))
        if correct / len(chunks) < 0.8:
            raise RuntimeError(f"Образец {names[ids[index]]} смешан с другим голосом. Выберите более чистый фрагмент.")
    similarity = centers @ centers.T
    for i in range(len(ids)):
        for j in range(i):
            if float(similarity[i, j]) > 0.8:
                raise RuntimeError(f"Образцы {names[ids[i]]} и {names[ids[j]]} слишком похожи. Проверьте, что в них говорят разные люди.")

    # Cache full-recording embeddings, independently of profiles, to make
    # subsequent corrections fast. Cache identity includes the actual files.
    audio_path = Path(meeting["audioPath"])
    cache_key = f"v1:{audio_path.stat().st_size}:{audio_path.stat().st_mtime_ns}:{model.stat().st_size}:{model.stat().st_mtime_ns}"
    cache = Path(args.meeting).parent / "calibration-features-v1.npz"
    features = None
    if cache.exists():
        try:
            with np.load(cache, allow_pickle=False) as saved:
                if str(saved["key"].item()) == cache_key:
                    features = saved["features"]
        except (ValueError, OSError, KeyError):
            pass
    count = math.ceil(duration)
    if features is None:
        features = np.zeros((count, extractor.dim), dtype=np.float32)
        for index in range(count):
            if index % max(1, count // 100) == 0:
                progress("calibrate", round(5 + index / count * 85, 1), "Сопоставляем речь с образцами участников…")
            center = index + 0.5
            start, end = max(0, center - 1.5), min(duration, center + 1.5)
            if end - start < 3:
                start, end = max(0, min(start, duration - 3)), min(duration, max(end, 3))
            vector = embedding(start, end)
            if vector is not None:
                features[index] = vector
        temporary = cache.with_suffix(".tmp.npz")
        np.savez_compressed(temporary, key=cache_key, features=features)
        temporary.replace(cache)
    scores = features @ centers.T
    turns = []
    unknown_seconds = 0
    for index, score in enumerate(scores):
        start, end = float(index), min(float(index + 1), duration)
        speaker = classify_similarity(score, ids)
        # Explicitly confirmed reference ranges are human-labelled data.
        for ref in references:
            if start >= ref["start"] and end <= ref["end"]:
                speaker = ref["speakerId"]
                break
        if speaker is None:
            unknown_seconds += end - start
        if turns and turns[-1]["speakerId"] == speaker:
            turns[-1]["end"] = end
        else:
            turns.append(dict(start=start, end=end, speakerId=speaker))
    segments = relabel_segments(meeting["segments"], turns)
    report = dict(model="3D-Speaker CAM++", references=references, referenceValidation=validation,
                  minimumSimilarity=0.5, minimumMargin=0.12, unknownSeconds=round(unknown_seconds, 2),
                  reviewSegments=sum(s["needsReview"] for s in segments), calibratedAt=__import__("datetime").datetime.now(__import__("datetime").timezone.utc).isoformat())
    atomic_json(args.output, dict(turns=turns, segments=segments, calibration=report))
    progress("calibrate", 100, "Голоса сопоставлены с подтверждёнными образцами")


def transcribe(args):
    turns = json.loads(Path(args.turns).read_text(encoding="utf-8"))
    rows = []
    with wave.open(args.audio) as audio, tempfile.TemporaryDirectory(prefix="ferrofluid-meeting-") as tmp:
        rate = audio.getframerate()
        duration = audio.getnframes() / rate
        count = math.ceil(duration / 120)
        for index in range(count):
            progress("transcribe", round(index / count * 100, 1), f"Распознавание речи · фрагмент {index + 1} из {count}")
            chunk = Path(tmp) / "chunk.wav"
            with wave.open(str(chunk), "wb") as output:
                output.setparams(audio.getparams())
                output.writeframes(audio.readframes(120 * rate))
            last_error = None
            base = Path(tmp) / "transcript"
            for engine in args.engines:
                base.with_suffix(".json").unlink(missing_ok=True)
                try:
                    run([engine, "-m", args.model, "-f", str(chunk), "-l", args.language,
                         "-ojf", "-of", str(base), "-ml", "1", "-sow", "-t", "4"])
                    result = json.loads(base.with_suffix(".json").read_text(encoding="utf-8"))
                    break
                except Exception as error:
                    last_error = error
            else:
                raise RuntimeError(str(last_error))
            rows.extend(align_segments(result, index * 120, turns))
    if not rows:
        raise RuntimeError("Whisper не распознал речь. Попробуйте другую модель или язык.")
    for index, row in enumerate(rows):
        row["id"] = f"segment-{index + 1}"
    atomic_json(args.output, rows)
    progress("transcribe", 100, "Транскрипт готов")


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("action", choices=["setup", "import", "analyze", "transcribe", "calibrate", "precise", "align"])
    for option in ["root", "source", "ffmpeg", "audio", "output", "model", "turns", "meeting", "references", "nemo"]:
        parser.add_argument(f"--{option}")
    parser.add_argument("--speakers", type=int, default=0)
    parser.add_argument("--language", default="auto")
    parser.add_argument("--engines", nargs="+", default=[])
    args = parser.parse_args()
    if args.action == "setup":
        setup(Path(args.root))
    elif args.action == "import":
        import_audio(args)
    elif args.action == "analyze":
        analyze(args)
    elif args.action == "calibrate":
        calibrate(args)
    elif args.action in ["precise", "align"]:
        from meetings_advanced import precise, align
        (precise if args.action == "precise" else align)(args)
    else:
        transcribe(args)


if __name__ == "__main__":
    try:
        main()
    except Exception as error:
        print(str(error), file=sys.stderr, flush=True)
        sys.exit(1)
