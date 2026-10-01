"""Qwen3-TTS sidecar. JSON-lines progress; all synthesis and references stay local."""
import argparse
import contextlib
from concurrent.futures import ThreadPoolExecutor
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import sys
import time
import urllib.request

MODELS = {
    "custom": "Qwen/Qwen3-TTS-12Hz-1.7B-CustomVoice",
    "base": "Qwen/Qwen3-TTS-12Hz-1.7B-Base",
}


def progress(message, percentage=0, **extra):
    print(json.dumps(dict(message=message, percentage=percentage, **extra), ensure_ascii=False), flush=True)


def chunks(text, limit=240):
    """Keep every word, split at sentence/word boundaries; cap each model request."""
    text = text.strip()
    if not text:
        raise ValueError("Введите текст для озвучивания.")
    if len(text) > 20000:
        raise ValueError("За один раз можно озвучить до 20 000 символов.")
    result = []
    current = ""
    for sentence in re.split(r"(?<=[.!?…])\s+|\n+", text):
        for word in sentence.split():
            while len(word) > limit:
                if current:
                    result.append(current)
                    current = ""
                result.append(word[:limit])
                word = word[limit:]
            if current and len(current) + len(word) + 1 > limit:
                result.append(current)
                current = ""
            current = (current + " " + word).strip()
        if current:
            result.append(current)
            current = ""
    # Pack short sentences together: fewer decode calls and more natural
    # cross-sentence prosody, while retaining a bounded request size.
    packed = []
    current = ""
    for part in result:
        if current and len(current) + len(part) + 1 > limit:
            packed.append(current)
            current = ""
        current = (current + " " + part).strip()
    if current:
        packed.append(current)
    return packed


def file_hash(path):
    digest = hashlib.sha256()
    with path.open("rb") as file:
        for block in iter(lambda: file.read(8 * 1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def parallel_windows_download(url, temporary, size, report):
    """Bounded ranges avoid restarting multi-GB files after CDN disconnects."""
    prefix = temporary.stat().st_size if temporary.exists() else 0
    if prefix >= size:
        return
    parts_dir = temporary.with_suffix(".parts")
    parts_dir.mkdir(exist_ok=True)
    ranges = [(start, min(size - 1, start + 64 * 1024 * 1024 - 1)) for start in range(prefix, size, 64 * 1024 * 1024)]
    def fetch(interval):
        start, end = interval
        part = parts_dir / f"{start}-{end}"
        if part.is_file() and part.stat().st_size == end - start + 1:
            return part
        for attempt in range(4):
            completed = subprocess.run(["curl.exe", "--location", "--fail", "--silent", "--show-error", "--connect-timeout", "30", "--max-time", "600", "--max-filesize", str(end - start + 1), "--range", f"{start}-{end}", "--output", str(part), url], creationflags=subprocess.CREATE_NO_WINDOW, stdout=sys.stderr, stderr=sys.stderr)
            if completed.returncode == 0 and part.stat().st_size == end - start + 1:
                return part
            time.sleep(2 ** attempt)
        raise RuntimeError("Загрузка оборвалась. Повторите установку: скачанные части сохранятся.")
    with ThreadPoolExecutor(max_workers=6) as pool:
        tasks = [pool.submit(fetch, interval) for interval in ranges]
        while not all(task.done() for task in tasks):
            report(prefix + sum(part.stat().st_size for part in parts_dir.iterdir() if part.is_file()))
            time.sleep(2)
        parts = [task.result() for task in tasks]
    assembly = temporary.with_suffix(".assembling")
    with assembly.open("wb") as output:
        if temporary.exists():
            with temporary.open("rb") as source:
                shutil.copyfileobj(source, output, 8 * 1024 * 1024)
        for part in parts:
            with part.open("rb") as source:
                shutil.copyfileobj(source, output, 8 * 1024 * 1024)
    # A crash while assembling leaves the original prefix and all complete
    # ranges intact. Resume never appends a second copy of an incomplete range.
    assembly.replace(temporary)
    for part in parts:
        part.unlink()
    parts_dir.rmdir()


def download_modelscope(root):
    """Official Qwen mirror, resumable downloads, hash-checked atomic promotion."""
    for index, (kind, repo) in enumerate(MODELS.items()):
        endpoint = "https://www.modelscope.cn"
        progress("Получаем список файлов " + repo.split("/")[-1] + "…", 35 + index * 30)
        with urllib.request.urlopen(endpoint + "/api/v1/models/" + repo + "/repo/files?Revision=master&Recursive=true", timeout=60) as response:
            manifest = json.load(response)
        if manifest.get("Code") != 200:
            raise RuntimeError("ModelScope не вернул список файлов модели.")
        files = [item for item in manifest["Data"]["Files"] if item["Type"] == "blob"]
        total = sum(item["Size"] for item in files)
        done = 0
        folder = root / "models" / kind
        for item in files:
            relative = Path(item["Path"])
            if relative.is_absolute() or ".." in relative.parts or ":" in str(relative):
                raise RuntimeError("Некорректный путь в списке файлов модели.")
            target = folder / relative
            target.parent.mkdir(parents=True, exist_ok=True)
            expected, size = item["Sha256"], item["Size"]
            if kind == "base" and item["Path"].startswith("speech_tokenizer/"):
                shared = root / "models" / "custom" / relative
                if shared.is_file() and shared.stat().st_size == size and file_hash(shared) == expected:
                    shutil.copyfile(shared, target)
            if target.is_file() and target.stat().st_size == size and file_hash(target) == expected:
                done += size
                continue
            temporary = target.with_suffix(target.suffix + ".download")
            last_emit = 0
            for attempt in range(4):
                try:
                    received = temporary.stat().st_size if temporary.exists() else 0
                    url = endpoint + "/api/v1/models/" + repo + "/repo?Revision=master&FilePath=" + item["Path"]
                    request = urllib.request.Request(url)
                    if received:
                        request.add_header("Range", f"bytes={received}-")
                    if received != size and os.name == "nt" and size > 128 * 1024 * 1024:
                        parallel_windows_download(url, temporary, size, lambda count: progress(f"{repo.split('/')[-1]} · {(done + count) / 1e9:.2f} / {total / 1e9:.2f} ГБ", 35 + index * 30 + (done + count) / total * 29))
                    elif received != size and os.name == "nt":
                        # Windows' Schannel transport handles local certificate/
                        # network middleware more reliably than Python OpenSSL.
                        args = ["curl.exe", "--location", "--fail", "--silent", "--show-error", "--connect-timeout", "30", "--max-time", "3600", "--speed-time", "90", "--speed-limit", "1024", "--continue-at", "-", "--output", str(temporary), request.full_url]
                        with subprocess.Popen(args, creationflags=subprocess.CREATE_NO_WINDOW, stdout=sys.stderr, stderr=sys.stderr) as download:
                            while download.poll() is None:
                                count = temporary.stat().st_size if temporary.exists() else 0
                                progress(f"{repo.split('/')[-1]} · {(done + count) / 1e9:.2f} / {total / 1e9:.2f} ГБ", 35 + index * 30 + (done + count) / total * 29)
                                time.sleep(2)
                            if download.returncode:
                                raise RuntimeError("Не удалось скачать " + item["Path"] + ". Повторите установку: загрузка продолжится.")
                    elif received != size:
                        with urllib.request.urlopen(request, timeout=120) as response:
                            if response.status != 206:
                                received = 0
                            with temporary.open("ab" if received else "wb") as output:
                                while True:
                                    block = response.read(1024 * 1024)
                                    if not block:
                                        break
                                    output.write(block)
                                    received += len(block)
                                    now = time.monotonic()
                                    if now - last_emit > 1:
                                        progress(f"{repo.split('/')[-1]} · {(done + received) / 1e9:.2f} / {total / 1e9:.2f} ГБ", 35 + index * 30 + (done + received) / total * 29)
                                        last_emit = now
                    if temporary.stat().st_size != size or file_hash(temporary) != expected:
                        temporary.unlink(missing_ok=True)
                        raise RuntimeError("Проверка целостности загрузки не прошла: " + item["Path"])
                    temporary.replace(target)
                    break
                except Exception:
                    if attempt == 3:
                        raise
                    time.sleep(2 ** attempt)
            done += size


def download_models(root):
    try:
        download_modelscope(root)
    except Exception as error:
        progress("ModelScope недоступен. Пробуем Hugging Face…", 35)
        from huggingface_hub import snapshot_download
        for index, (kind, repo) in enumerate(MODELS.items()):
            progress("Скачиваем " + repo.split("/")[-1] + "…", 35 + index * 30)
            snapshot_download(repo, local_dir=root / "models" / kind)


def setup(root):
    import torch
    if not torch.cuda.is_available():
        raise RuntimeError("CUDA недоступна. Обновите драйвер NVIDIA и повторите установку.")
    # Verify a real operation, not merely device enumeration (important for SM120).
    torch.ones(8, device="cuda").sum().item()
    download_models(root)
    from qwen_tts import Qwen3TTSModel  # validate dependencies before marking ready
    marker = root / "ready.json"
    temp = marker.with_suffix(".tmp")
    temp.write_text(json.dumps(dict(version=1, gpu=torch.cuda.get_device_name(0))), encoding="utf-8")
    temp.replace(marker)
    progress("Qwen готов · " + torch.cuda.get_device_name(0), 100)


def synthesize(root, request):
    started = time.monotonic()
    import numpy as np
    import soundfile as sf
    import torch
    from qwen_tts import Qwen3TTSModel
    if not torch.cuda.is_available():
        raise RuntimeError("Qwen требует доступную видеокарту NVIDIA с CUDA.")
    parts = chunks(request["text"])
    clone = bool(request.get("reference"))
    kind = "base" if clone else "custom"
    progress("Загружаем Qwen на видеокарту…", 2)
    # SDPA works on Windows/Blackwell without a separate FlashAttention compiler.
    with contextlib.redirect_stdout(sys.stderr):
        model = Qwen3TTSModel.from_pretrained(
            str(root / "models" / kind), device_map="cuda:0",
            dtype=torch.bfloat16, attn_implementation="sdpa",
            local_files_only=True,
        )
        prompt = None
        if clone:
            prompt = model.create_voice_clone_prompt(
                ref_audio=request["reference"], ref_text=request["referenceText"],
                x_vector_only_mode=False,
            )
    destination = Path(request["output"])
    destination.parent.mkdir(parents=True, exist_ok=True)
    temporary = destination.with_suffix(".partial.wav")
    total = 0
    try:
        with sf.SoundFile(temporary, mode="w", samplerate=24000, channels=1, subtype="PCM_16") as output:
            for index, part in enumerate(parts):
                progress(f"Озвучиваем фрагмент {index + 1} из {len(parts)}…", 5 + index / len(parts) * 90)
                with torch.inference_mode(), contextlib.redirect_stdout(sys.stderr):
                    if clone:
                        audio, sr = model.generate_voice_clone(text=part, language="Russian", voice_clone_prompt=prompt, max_new_tokens=2048)
                    else:
                        audio, sr = model.generate_custom_voice(text=part, language="Russian", speaker=request["speaker"], instruct=request.get("style", ""), max_new_tokens=2048)
                if sr != 24000:
                    raise RuntimeError(f"Неожиданная частота Qwen: {sr}")
                samples = np.asarray(audio[0], dtype=np.float32).reshape(-1)
                if not len(samples) or not np.isfinite(samples).all():
                    raise RuntimeError("Модель вернула пустое или повреждённое аудио.")
                output.write(samples)
                total += len(samples)
                if index + 1 < len(parts):
                    output.write(np.zeros(int(sr * .18), dtype=np.float32))
        temporary.replace(destination)
    except BaseException:
        temporary.unlink(missing_ok=True)
        raise
    progress("Озвучивание готово", 100, audioPath=str(destination), durationSeconds=total / 24000, gpu=torch.cuda.get_device_name(0), elapsedSeconds=round(time.monotonic() - started, 2), peakVramMiB=round(torch.cuda.max_memory_allocated() / 1024 ** 2))


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("mode", choices=["setup", "synthesize", "download"])
    parser.add_argument("--root", type=Path, required=True)
    args = parser.parse_args()
    os.environ["HF_HOME"] = str(args.root / "cache")
    os.environ["HF_HUB_DISABLE_TELEMETRY"] = "1"
    os.environ["HF_HUB_DISABLE_PROGRESS_BARS"] = "1"
    if args.mode == "download":
        download_models(args.root)
    elif args.mode == "setup":
        setup(args.root)
    else:
        os.environ["HF_HUB_OFFLINE"] = "1"
        os.environ["TRANSFORMERS_OFFLINE"] = "1"
        synthesize(args.root, json.loads(sys.stdin.read()))


if __name__ == "__main__":
    try:
        main()
    except Exception as error:
        progress(str(error), error=True)
        raise
