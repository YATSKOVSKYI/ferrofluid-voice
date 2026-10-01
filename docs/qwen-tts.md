# Qwen voice studio

Library → Озвучивание → Qwen. Ready-made voices use
`Qwen/Qwen3-TTS-12Hz-1.7B-CustomVoice`; saved voice references use
`Qwen/Qwen3-TTS-12Hz-1.7B-Base`. Both models are Apache-2.0.
The existing Piper/Silero screen remains available via its own button.

Select one of nine speakers or click **Клонировать голос**. Import a 3–30 second
WAV/M4A/MP3/FLAC/OGG recording, enter a name and the exact spoken text. Reference
files are copied and converted to mono 24 kHz WAV; deleting the source does not
break the saved profile. A longer recording is rejected instead of silently
truncating it and mismatching the transcript. For Russian, use a clean native
speaker reference. Qwen's ready-made speakers are multilingual, but none is
documented as a native Russian speaker.

Select a voice, enter text, generate, listen with the audio controls, or export
WAV. Long text is divided into bounded sentences/word groups (up to 20,000
characters per request). CustomVoice offers speaking-style instructions; Base
uses the reference's speaking style. A reference creates an inference prompt;
this is zero-shot cloning, not model training.

The isolated environment and models live in `%APPDATA%/Ferrofluid Voice/tts/qwen`;
outputs live in `tts/output`. Setup uses uv-managed Python 3.12, Qwen TTS 0.1.1
and PyTorch/torchaudio 2.8.0 CUDA 12.9. Expect roughly 13 GB downloaded and up
to 20 GB storage, including runtime dependencies. Windows uses PyTorch SDPA,
without building FlashAttention. No automatic CPU fallback hides CUDA failures.

Setup downloads official Qwen weights from ModelScope, checking SHA-256 and
file sizes before promoting temporary files. Large Windows downloads use six
bounded curl ranges and retain completed parts after interruption. Hugging Face
is the fallback source. Audio, reference transcripts and generated speech stay
local; only initial package/model installation needs the network. Inference
enforces Hugging Face/Transformers offline mode.

One Qwen job may run at a time across application windows. Blocking work runs
off Tauri's UI thread and reports progress. Cancellation kills the Windows job
tree; closing/crashing the app also kills it. Partial WAVs never replace a
successful output. The worker exits after each generation, freeing GPU memory;
therefore first-generation/model loading adds several seconds to every request.
Setup has bounded timeouts; interrupted downloads can be retried. Window changes
recover the most recent completed audio from job status.

Validation:

- `python tests/test_qwen_worker.py`: text boundaries and limits.
- Rust `qwen` unit tests: exclusive jobs, path validation, real worker cleanup.
- Browser checks with mock IPC: voice selection, adding/deleting profiles,
  light/dark themes, and a narrow window. Mock IPC does not verify the model.
- Real CUDA synthesis checks run separately against installed model weights.

Local validation on RTX 5080 (16 GB), driver 616.64:
Ryan produced 6.48 s audio in 23.78 s with peak PyTorch allocation 4107 MiB;
Serena produced 5.92 s audio in 20.42 s with 4168 MiB. Times include Python/model
loading. Whisper Medium recognized the Russian words in the generated sample;
this is an intelligibility check, not a human naturalness score or a universal
speed benchmark. Reference-based cloning is checked separately.
Base cloned the generated Serena test reference and produced a new 6.88 s
Russian utterance in 23.94 s, peak allocation 4424 MiB; Whisper Medium recovered
the new sentence. This verifies the reference-based synthesis path, not the
similarity of a particular user's voice. Both model downloads passed their
official ModelScope SHA-256 checks before setup was marked ready.
