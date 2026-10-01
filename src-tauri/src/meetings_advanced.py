"""Local turn detection and forced alignment. Imported by meetings_worker."""
import json
import hashlib
import os
from pathlib import Path
import re
import subprocess
import sys
import wave

RUSSIAN_REPO = "https://huggingface.co/jonatasgrosman/wav2vec2-large-xlsr-53-russian/resolve/2329100508896c6d9b157019803ab5601e6f3406"
RUSSIAN_HASH = "d1cdb1a7921de7d363f967a9b0101a713602e109dba62b6f3f9ae2e0b2df0c1c"


def file_sha256(path):
    digest = hashlib.sha256()
    with path.open("rb") as source:
        for block in iter(lambda: source.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def checked_download(url, destination, expected_hash=None):
    from meetings_worker import run
    import urllib.request
    destination.parent.mkdir(parents=True, exist_ok=True)
    if not destination.exists():
        temporary = destination.with_suffix(destination.suffix + ".download")
        if expected_hash:
            run(["curl.exe", "-L", "--fail", "--retry", "10", "--retry-all-errors", "--connect-timeout", "30", "--speed-time", "60", "--speed-limit", "1000", "-C", "-", "-o", str(temporary), url])
        else:
            with urllib.request.urlopen(url, timeout=180) as response:
                temporary.write_bytes(response.read())
        if expected_hash and file_sha256(temporary) != expected_hash:
            raise RuntimeError("Скачанная модель не прошла проверку SHA-256. Повторите загрузку.")
        temporary.replace(destination)
    if expected_hash and file_sha256(destination) != expected_hash:
        raise RuntimeError("Локальная модель повреждена: SHA-256 не совпадает.")


def canonical_text(segments):
    return " ".join(" ".join(s["text"].split()) for s in segments)


def ensure_alignment(root):
    from meetings_worker import run, progress
    runtime = root / "alignment-runtime"
    python = runtime / ("Scripts/python.exe" if os.name == "nt" else "bin/python")
    ready = runtime / "alignment-ready-v1"
    if ready.exists() and python.is_file():
        return python
    progress("setup", 0, "Подготовка выравнивания слов · первая установка может занять несколько минут…")
    if not python.is_file():
        run([sys._base_executable, "-m", "venv", str(runtime)])
    run([str(python), "-m", "pip", "install", "--upgrade", "pip", "--timeout", "180", "--disable-pip-version-check"])
    run([str(python), "-m", "pip", "install", "torch==2.8.0", "torchaudio==2.8.0", "--index-url", "https://download.pytorch.org/whl/cpu", "--timeout", "180", "--retries", "5", "--disable-pip-version-check"])
    run([str(python), "-m", "pip", "install", "numpy==2.2.6", "pandas==2.3.3", "scipy==1.16.3", "nltk==3.9.2", "transformers==4.57.3", "huggingface-hub==0.36.0", "--timeout", "180", "--disable-pip-version-check"])
    # Alignment imports only; no ASR/cloud/pyannote runtime is needed here.
    run([str(python), "-m", "pip", "install", "whisperx==3.8.6", "--no-deps", "--timeout", "180", "--disable-pip-version-check"])
    run([str(python), "-c", "from whisperx.alignment import load_align_model, align"])
    ready.write_text("whisperx=3.8.6;torch=2.8.0;transformers=4.57.3", encoding="utf-8")
    return python


def aligned_rows(original, aligned, turns):
    """Keep every source word, abstain on missing timing, preserve manual locks."""
    from meetings_worker import assign_speaker
    output = []
    for source, result in zip(original, aligned):
        spans = list(re.finditer(r"\S+", source["text"]))
        words = result.get("words", [])
        if source.get("speakerLocked"):
            output.append(dict(source))
            continue
        if len(words) != len(spans) or any(w.get("word", "").strip() != m.group() for w, m in zip(words, spans)):
            output.append({**source, "speakerId": None, "needsReview": True})
            continue
        groups = []
        for word, span in zip(words, spans):
            start, end = word.get("start"), word.get("end")
            if start is None or end is None or end <= start:
                # Retain the whole original sentence if even one word lacks timing.
                groups = []
                break
            speaker, review = assign_speaker(start, end, turns)
            if review:
                speaker = None
            if groups and groups[-1]["speakerId"] == speaker and start - groups[-1]["end"] < 1.2:
                groups[-1]["end"] = end
                groups[-1]["last"] = span.end()
                groups[-1]["needsReview"] |= review
            else:
                groups.append(dict(start=start, end=end, speakerId=speaker, needsReview=review, first=span.start(), last=span.end()))
        if not groups:
            output.append({**source, "speakerId": None, "needsReview": True})
        else:
            for index, group in enumerate(groups):
                first, last = group.pop("first"), group.pop("last")
                output.append(dict(id=f"{source['id']}-aligned-{index+1}", text=source["text"][first:last], speakerLocked=False, **group))
    if len(original) != len(aligned) or canonical_text(output) != canonical_text(original):
        raise RuntimeError("Выравнивание не должно терять или заменять слова исходного транскрипта.")
    return output


def overlap_regions(turns):
    events = []
    for t in turns:
        events.extend([(t["start"], 1, t["speakerId"]), (t["end"], -1, t["speakerId"])])
    active, result, previous = {}, [], 0
    for time, change, speaker in sorted(events):
        if len(active) > 1 and time > previous:
            if result and result[-1]["end"] == previous:
                result[-1]["end"] = time
            else:
                result.append(dict(start=previous, end=time))
        active[speaker] = active.get(speaker, 0) + change
        if active[speaker] == 0:
            del active[speaker]
        previous = time
    return result


def align(args):
    import numpy as np
    import torch
    from whisperx.alignment import load_align_model, align as forced_align
    from meetings_worker import atomic_json, progress
    torch.set_num_threads(min(8, os.cpu_count() or 2))
    meeting = json.loads(Path(args.meeting).read_text(encoding="utf-8"))
    language = args.language if args.language != "auto" else meeting["language"]
    if language == "auto":
        raise RuntimeError("Для выравнивания выберите язык конференции.")
    with wave.open(meeting["audioPath"]) as f:
        samples = np.frombuffer(f.readframes(f.getnframes()), dtype="<i2").astype(np.float32) / 32768
    model_dir = Path(args.root) / "alignment-models"
    model_dir.mkdir(exist_ok=True)
    os.environ["NLTK_DATA"] = str(Path(args.root) / "nltk-data")
    import nltk
    nltk.data.path.insert(0, os.environ["NLTK_DATA"])
    os.environ["HF_HUB_DISABLE_XET"] = "1"
    os.environ["HF_HUB_DISABLE_PROGRESS_BARS"] = "1"
    progress("align", 52, "Загружаем локальную модель границ слов…")
    model_name = None
    if language == "ru":
        russian = model_dir / "russian"
        for name in ["config.json", "preprocessor_config.json", "special_tokens_map.json", "vocab.json"]:
            checked_download(f"{RUSSIAN_REPO}/{name}", russian / name)
        checked_download(f"{RUSSIAN_REPO}/pytorch_model.bin", russian / "pytorch_model.bin", RUSSIAN_HASH)
        model_name = str(russian)
    model, metadata = load_align_model(language, "cpu", model_name=model_name, model_dir=str(model_dir))
    # A single whole-meeting waveform and absolute source timestamps avoid
    # reset/offset mistakes. Batch small groups for progress and cancellation.
    segments = []
    source = meeting["segments"]
    for index, segment in enumerate(source):
        progress("align", round(55 + index / max(1, len(source)) * 40, 1), f"Уточняем слова · реплика {index+1} из {len(source)}")
        if segment.get("speakerLocked"):
            segments.append(dict(words=[]))
            continue
        result = forced_align([dict(start=segment["start"], end=segment["end"], text=segment["text"])], model, metadata, samples, "cpu", interpolate_method="ignore", return_char_alignments=False)
        segments.append(dict(words=result["word_segments"]))
    atomic_json(args.output, segments)


def precise(args):
    from meetings_worker import atomic_json, progress, run, classify_similarity, FLAGS
    import numpy as np
    import sherpa_onnx as sherpa
    root = Path(args.root)
    meeting = json.loads(Path(args.meeting).read_text(encoding="utf-8"))
    refs = (meeting.get("calibration") or {}).get("references", [])
    if not refs or not meeting["segments"]:
        raise RuntimeError("Сначала подтвердите образцы всех участников и создайте транскрипт.")
    directory = Path(args.meeting).parent
    cli = Path(args.nemo) if args.nemo else root / "nemo" / "current" / "bin" / "nemo-speech.exe"
    if not cli.is_file():
        raise RuntimeError("Точный движок NVIDIA не найден. Используйте настольную сборку Ferrofluid Voice с компонентом конференций.")
    model = root / "nemo" / "Nemotron-3-Diarization.q8_0.gguf"
    checked_download("https://huggingface.co/nvidia/Nemotron-3-Diarization/resolve/f667ed73aee57d40cc39428eb768b4fd87a0a29e/Nemotron-3-Diarization.q8_0.gguf", model, "08456d9e22cd9a323c0364d98375f3746d6e68507ebb705cd46438c534c7a3a1")
    raw_path = directory / "precise-diarization.json"
    identity_path = directory / "precise-cache-key.json"
    audio_stat = Path(meeting["audioPath"]).stat()
    model_stat = model.stat()
    keys = dict(audio=f"{audio_stat.st_size}:{audio_stat.st_mtime_ns}:{model_stat.st_size}:{model_stat.st_mtime_ns}:{cli.stat().st_mtime_ns}:v3-offline",
                alignment=hashlib.sha256(json.dumps([args.language, meeting["segments"]], ensure_ascii=False, sort_keys=True).encode()).hexdigest())
    old_keys = json.loads(identity_path.read_text()) if identity_path.exists() else {}
    if not raw_path.exists() or old_keys.get("audio") != keys["audio"]:
        progress("precise", 5, "Определяем смены говорящих и одновременную речь…")
        run([str(cli), "diarize", meeting["audioPath"], "--model", str(model), "--preset", "v3-offline", "--device", "auto", "--format", "json", "--output", str(raw_path), "--force"])
    raw = json.loads(raw_path.read_text(encoding="utf-8"))
    raw_turns = raw["segments"]
    turns = [dict(start=float(t["start"]), end=float(t["end"]), speakerId=str(t["speaker"])) for t in raw_turns]
    overlaps = overlap_regions(turns)
    with wave.open(meeting["audioPath"]) as f:
        samples = np.frombuffer(f.readframes(f.getnframes()), dtype="<i2").astype(np.float32) / 32768
    extractor = sherpa.SpeakerEmbeddingExtractor(sherpa.SpeakerEmbeddingExtractorConfig(model=str(root / "campplus.onnx"), num_threads=2))

    def vector(start, end):
        clip = np.ascontiguousarray(samples[int(start*16000):int(end*16000)])
        if len(clip) < 16000 or np.sqrt(np.mean(clip**2)) < 0.0002:
            return None
        stream = extractor.create_stream()
        stream.accept_waveform(sample_rate=16000, waveform=clip)
        stream.input_finished()
        if not extractor.is_ready(stream):
            return None
        v = np.asarray(extractor.compute(stream), dtype=np.float32)
        return v / max(float(np.linalg.norm(v)), 1e-9)

    ids = list(dict.fromkeys(r["speakerId"] for r in refs))
    profiles = []
    for speaker in ids:
        values = [vector(float(start), float(start+3)) for r in refs if r["speakerId"] == speaker for start in np.arange(r["start"]+.5, r["end"]-3, 3)]
        values = [v for v in values if v is not None]
        if len(values) < 2:
            raise RuntimeError("В подтверждённом образце недостаточно чистой речи.")
        mean = np.mean(values, axis=0)
        profiles.append(mean / np.linalg.norm(mean))
    profiles = np.stack(profiles)
    # Map global neural channels to names using only human-confirmed anchors.
    evidence = {}
    for t in turns:
        for r in refs:
            amount = max(0, min(t["end"], r["end"]) - max(t["start"], r["start"]))
            if amount:
                weights = evidence.setdefault(t["speakerId"], {})
                weights[r["speakerId"]] = weights.get(r["speakerId"], 0) + amount
    mapping = {}
    for label, weights in evidence.items():
        ordered = sorted(weights, key=weights.get, reverse=True)
        if weights[ordered[0]] / sum(weights.values()) >= .8:
            mapping[label] = ordered[0]
    named = []
    for index, t in enumerate(turns):
        if index % 20 == 0:
            progress("precise", round(20 + index / len(turns) * 25, 1), "Проверяем голоса по подтверждённым образцам…")
        chunks = []
        for start in np.arange(t["start"], t["end"]-1, 3):
            end = min(float(start+3), t["end"])
            if any(start < o["end"] and end > o["start"] for o in overlaps):
                continue
            v = vector(float(start), end)
            if v is not None:
                chunks.append(v)
        if chunks:
            scores = np.mean(chunks, axis=0) @ profiles.T
            identity = classify_similarity(scores, ids)
        else:
            identity = mapping.get(t["speakerId"])
        named.append(dict(start=t["start"], end=t["end"], speakerId=identity))
    # Unknown regions must not suppress known overlapping activity in display;
    # label words in any detected overlap explicitly as uncertain.
    align_python = ensure_alignment(root)
    aligned_path = directory / "precise-words.json"
    if not aligned_path.exists() or old_keys.get("alignment") != keys["alignment"]:
        command = [str(align_python), str(Path(__file__).with_name("meetings_worker.py")), "align", "--root", str(root), "--meeting", args.meeting, "--output", str(aligned_path), "--language", args.language]
        process = subprocess.Popen(command, stdout=subprocess.PIPE, text=True, encoding="utf-8", **FLAGS)
        for line in process.stdout:
            print(line.rstrip(), flush=True)
        if process.wait():
            raise RuntimeError("Не удалось уточнить границы слов. Исходная конференция сохранена.")
    aligned = json.loads(aligned_path.read_text(encoding="utf-8"))
    atomic_json(identity_path, keys)
    # Add an unknown competitor on overlap intervals so assign_speaker cannot
    # confidently attribute simultaneous voices to a single named person.
    rows = aligned_rows(meeting["segments"], aligned, named + [dict(**o, speakerId=None) for o in overlaps])
    report = dict(engine="NVIDIA Nemotron-3-Diarization", alignment="WhisperX / wav2vec2 CTC", references=refs,
                  overlaps=overlaps, reviewSegments=sum(s["needsReview"] for s in rows), sourceMeetingId=meeting["id"],
                  calibratedAt=__import__("datetime").datetime.now(__import__("datetime").timezone.utc).isoformat())
    atomic_json(args.output, dict(turns=named, segments=rows, calibration=report))
    progress("precise", 100, "Готова новая версия с уточнёнными границами слов")
