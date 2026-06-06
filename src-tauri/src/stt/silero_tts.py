"""Silero TTS sidecar for Ferrofluid Voice.

Runs a user-supplied Silero `.pt` (torch.package) model using a system Python
that already has PyTorch installed. The Rust side talks to this script over
stdin/stdout with a single JSON message, so no extra Python dependencies
(numpy, torchaudio, soundfile) are required -- only `torch`.

Protocol:
  stdin  -> {"mode": "speakers"|"synthesize", "model": "...", ...}
  stdout -> {"ok": true, ...} on success, {"ok": false, "error": "..."} on failure
"""

import json
import struct
import sys
import wave


def _fail(message):
    sys.stdout.write(json.dumps({"ok": False, "error": str(message)}))
    sys.stdout.flush()
    sys.exit(1)


def _load_model(torch, model_path):
    # Silero ru models (v3/v4/v5) are shipped as torch.package archives and are
    # loaded by un-pickling the "model" object from the "tts_models" package.
    importer = torch.package.PackageImporter(model_path)
    model = importer.load_pickle("tts_models", "model")
    model.to(torch.device("cpu"))
    return model


def main():
    try:
        request = json.loads(sys.stdin.read() or "{}")
    except Exception as error:  # noqa: BLE001
        _fail("Invalid request: {}".format(error))
        return

    try:
        import torch  # noqa: F401
    except Exception as error:  # noqa: BLE001
        _fail(
            "PyTorch is not available in this Python ({}). Install it with "
            "`pip install torch`. Original error: {}".format(sys.executable, error)
        )
        return

    model_path = request.get("model")
    if not model_path:
        _fail("No model path supplied.")
        return

    mode = request.get("mode", "synthesize")

    try:
        model = _load_model(torch, model_path)
    except Exception as error:  # noqa: BLE001
        _fail("Could not load Silero model: {}".format(error))
        return

    if mode == "speakers":
        speakers = list(getattr(model, "speakers", []) or [])
        sys.stdout.write(json.dumps({"ok": True, "speakers": speakers}))
        sys.stdout.flush()
        return

    text = (request.get("text") or "").strip()
    if not text:
        _fail("No text supplied.")
        return

    output_path = request.get("output")
    if not output_path:
        _fail("No output path supplied.")
        return

    sample_rate = int(request.get("sampleRate", 48000))
    speaker = request.get("speaker") or None

    try:
        kwargs = {"text": text, "sample_rate": sample_rate, "put_accent": True, "put_yo": True}
        if speaker:
            kwargs["speaker"] = speaker
        audio = model.apply_tts(**kwargs)
    except Exception as error:  # noqa: BLE001
        _fail("Silero synthesis failed: {}".format(error))
        return

    try:
        samples = (audio.clamp(-1.0, 1.0) * 32767.0).round().to(torch.int16).tolist()
        pcm = struct.pack("<{}h".format(len(samples)), *samples)
        with wave.open(output_path, "wb") as handle:
            handle.setnchannels(1)
            handle.setsampwidth(2)
            handle.setframerate(sample_rate)
            handle.writeframes(pcm)
    except Exception as error:  # noqa: BLE001
        _fail("Could not write audio file: {}".format(error))
        return

    sys.stdout.write(json.dumps({"ok": True, "output": output_path, "speaker": speaker or ""}))
    sys.stdout.flush()


if __name__ == "__main__":
    main()
