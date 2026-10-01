# Local validation — 1 October 2026

Validated on Windows with the actual Tauri release build, not a simulated API.

- Cloned `YATSKOVSKYI/ferrofluid-voice` into `C:/projects/ferrofluid-voice`.
- TypeScript + Vite production build, Rust check, and native release build pass.
- Six Python regression tests pass: overlap review, uncovered speech, timed speaker changes, chunk offsets, untimed-word preservation, nearby provisional labels and grouped review flags.
- Imported the supplied Zoom M4A (AAC stereo, 48 kHz) successfully. Working audio is 1,919.019 seconds, PCM16 mono 16 kHz. The source file was not modified.
- Analyzed the full recording with the user's confirmed count of four participants. Four voice groups are saved with playback samples and provisional names.
- Transcribed all sixteen two-minute chunks with the existing local Medium model. Saved 205 grouped segments, covering the final second of the recording. This is a machine draft: identities and marked fragments require the user's review.
- Native playback works through the scoped Tauri asset protocol, including the complete 32-minute working file.
- Renamed and saved a speaker, then re-read the stored meeting; the name persisted.
- Added a manual speaker and merged another speaker's turns and transcript labels; the changes persisted. Invalid meeting IDs are rejected.
- Split a transcript segment at the audio playhead and text cursor through the actual UI, then saved it. Text search was checked through the actual UI as well.
- Cancelled a running native transcription. The process stopped, busy status cleared, and the saved transcript remained intact.
- TXT/SRT/JSON export formatting contains speaker names and timestamps; millisecond carry to the next hour was checked.
- UI screenshots checked at 1100×780 in light and dark mode and at 880×640. No horizontal overflow in the conference content at either size.

Private recordings, voice models, transcripts and test artifacts are outside the Git change set.

## Speaker calibration correction

- The initial forced four-group clustering was not accurate on the supplied mixed Zoom recording. Generic group IDs were not reliable identities.
- Compared reference-window separability using local WeSpeaker, CAM++ and TitaNet models; selected CAM++ after all 51 reference windows matched their named profile under leave-one-window-out validation. This is validation of supplied samples, not a full-recording accuracy measure.
- User-confirmed samples: Олеся 03:14–03:43, Дима 03:45–05:28, Саша 24:41–24:58, Миша 14:14–14:25 (interpreted from the user's abbreviated end time).
- Ran calibration through the actual release UI, filled all four names/time ranges, and successfully persisted the native command's result.
- All 205 transcript IDs, text strings, starts and ends remained exactly unchanged. 140 segments received a named label; 65 remained unassigned and marked for review. Full-recording identities still require listening/review, particularly overlapping or brief speech.
- Reference-window validation: Олеся 9/9, Дима 34/34, Саша 5/5, Миша 3/3. Confirmed reference intervals are explicitly human-labelled.
- Eight Python regression tests pass, including low/marginal-match abstention, mixed-sentence review, unchanged text and preserved manual assignments. TypeScript production and native release builds pass.
- A complete before-calibration JSON backup was saved. Repeat calibration reused the local embedding cache and completed within a few seconds.
- Native UI rejects empty ranges, overly long samples and overlaps between different participants. Calibration panel was visually checked in light mode and dark mode at 880×640, without horizontal overflow.

## Full turn detection and word alignment

- Community-1 model access was tested with the configured Hugging Face credential; HTTP 403 prevented downloading its gated weights. No audio was uploaded.
- Official NeMo-Speech.cpp 0.1.0 Windows binaries do not support the Nemotron-3 pre-layer-norm checkpoint. Built the official source at `4c101bc7113f49101a3e11d2c994c519f41939f6` with CUDA 13.3, architecture 120, on the installed RTX 5080. Full-recording inference succeeded with both Sortformer V2 and Nemotron-3 using the compatible runtimes.
- Compared anonymous-channel naming using the first half of each user reference and scoring the second half. Sortformer V2 merged Олеся with a male voice; Nemotron-3 maintained four distinct dominant channels on those references. This small reference check does not constitute full-meeting DER or a general model ranking.
- Ran **Уточнить реплики** through the actual release UI on the complete 1,919-second recording. NVIDIA returned 339 turns and 98 predicted overlap regions. CAM++ checks clean detected turns against confirmed profiles; overlapping words remain provisional.
- WhisperX 3.8.6 with the Russian wav2vec2 model force-aligned the existing text. Created `meeting-1790823744411838-refined` as a separate version with 399 transcript segments. All 3,526 whitespace-delimited source words are preserved in the same order. The original meeting metadata remained identical to the input snapshot.
- Named-word coverage increased from 2,663/3,526 to 3,155/3,526; 371 words remain unassigned versus 863 in the prior version. Coverage measures assignment, not correctness. No full-recording accuracy percentage is claimed.
- Eleven regression tests pass, including exact source-word preservation across speaker changes, conservative fallback for missing word times, overlap detection and preservation of manual locks. Production TypeScript and native release builds pass.
- Cancelled a native refinement job while NVIDIA was processing; its process tree stopped, busy status cleared, and both saved meeting versions remained exactly unchanged.
- Native UI shows the selected engine and predicted overlap count. Screenshot of the final action card checked in dark mode.
- Model downloads were verified with SHA-256. Python dependencies are isolated in the application's private environment. Temporary compiler files were compressed with NTFS after disk space ran low; no recordings or user files were deleted.
