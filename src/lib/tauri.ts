import { convertFileSrc, invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import type {
  HistoryItem,
  Language,
  ModelStatus,
  TranscriptResult,
  TtsCustomModelInfo,
  TtsStatus,
  TtsSynthesisResult,
  TtsVoiceInfo,
  WhisperModelInfo,
} from "./types";

declare global {
  interface Window {
    __TAURI_INTERNALS__?: unknown;
  }
}

function hasTauriRuntime() {
  return typeof window !== "undefined" && Boolean(window.__TAURI_INTERNALS__);
}

function requireTauriRuntime() {
  if (!hasTauriRuntime()) {
    throw new Error("Tauri runtime is not available. Run the desktop app with npm run tauri:dev.");
  }
}

export function getModelStatus() {
  if (!hasTauriRuntime()) {
    return Promise.resolve<ModelStatus>({ exists: false, backend: "auto", engineExists: false, gpuEngineExists: false });
  }
  return invoke<ModelStatus>("get_model_status");
}

export function setModelPath(path: string) {
  requireTauriRuntime();
  return invoke<ModelStatus>("set_model_path", { path });
}

export function openModelsFolder() {
  requireTauriRuntime();
  return invoke<void>("open_models_folder");
}

export function startWindowDrag() {
  requireTauriRuntime();
  return getCurrentWindow().startDragging();
}

export function closeCurrentWindow() {
  if (!hasTauriRuntime()) {
    window.close();
    return Promise.resolve();
  }
  return invoke<void>("close_current_window");
}

export function openSettingsWindow() {
  if (!hasTauriRuntime()) {
    window.open(`${window.location.origin}${window.location.pathname}?view=settings`, "voiceglass-settings", "width=1040,height=760");
    return Promise.resolve();
  }
  return invoke<void>("open_settings_window");
}

export function openLibraryWindow() {
  if (!hasTauriRuntime()) {
    window.open(`${window.location.origin}${window.location.pathname}?view=library`, "voiceglass-library", "width=1100,height=780");
    return Promise.resolve();
  }
  return invoke<void>("open_library_window");
}

export function listWhisperModels() {
  if (!hasTauriRuntime()) {
    return Promise.resolve<WhisperModelInfo[]>([
      {
        id: "tiny",
        name: "Tiny",
        fileName: "ggml-tiny.bin",
        size: "75 MB",
        description: "Fastest option for short notes and quick commands.",
        url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-tiny.bin",
        isDownloaded: false,
        isSelected: false,
      },
      {
        id: "base",
        name: "Base",
        fileName: "ggml-base.bin",
        size: "142 MB",
        description: "Good default for lightweight local transcription.",
        url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-base.bin",
        isDownloaded: false,
        isSelected: false,
      },
      {
        id: "small",
        name: "Small",
        fileName: "ggml-small.bin",
        size: "466 MB",
        description: "Better accuracy while still practical on most laptops.",
        url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-small.bin",
        isDownloaded: false,
        isSelected: false,
      },
      {
        id: "medium",
        name: "Medium",
        fileName: "ggml-medium.bin",
        size: "1.5 GB",
        description: "Higher quality for longer dictation and mixed speech.",
        url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-medium.bin",
        isDownloaded: false,
        isSelected: false,
      },
      {
        id: "large-v3-turbo",
        name: "Large v3 Turbo",
        fileName: "ggml-large-v3-turbo.bin",
        size: "1.6 GB",
        description: "Strong quality with better speed than the full large model.",
        url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-large-v3-turbo.bin",
        isDownloaded: false,
        isSelected: false,
      },
      {
        id: "large-v3",
        name: "Large v3",
        fileName: "ggml-large-v3.bin",
        size: "3.1 GB",
        description: "Best quality option, needs more disk and compute.",
        url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-large-v3.bin",
        isDownloaded: false,
        isSelected: false,
      },
    ]);
  }
  return invoke<WhisperModelInfo[]>("list_whisper_models");
}

export function downloadWhisperModel(modelId: string) {
  requireTauriRuntime();
  return invoke<ModelStatus>("download_whisper_model", { modelId });
}

export function getDownloadProgress(modelId: string) {
  if (!hasTauriRuntime()) {
    return Promise.resolve<any>(null);
  }
  return invoke<any>("get_download_progress", { modelId });
}

export function cancelDownload() {
  requireTauriRuntime();
  return invoke<void>("cancel_download");
}

export function getTtsStatus() {
  if (!hasTauriRuntime()) {
    return Promise.resolve<TtsStatus>({
      engineExists: true,
      enginePath: "built-in Piper TTS",
      selectedVoiceDownloaded: false,
    });
  }
  return invoke<TtsStatus>("get_tts_status");
}

export function listTtsVoices() {
  if (!hasTauriRuntime()) {
    return Promise.resolve<TtsVoiceInfo[]>([
      {
        id: "ru_RU-dmitri-medium",
        name: "Dmitri",
        locale: "ru_RU",
        quality: "Medium",
        size: "~63 MB",
        description: "Russian male Piper voice from rhasspy/piper-voices.",
        license: "MIT",
        repository: "rhasspy/piper-voices",
        url: "https://huggingface.co/rhasspy/piper-voices/tree/main/ru/ru_RU/dmitri/medium",
        isDownloaded: false,
        isSelected: false,
      },
      {
        id: "ru_RU-denis-medium",
        name: "Denis",
        locale: "ru_RU",
        quality: "Medium",
        size: "~63 MB",
        description: "Alternative Russian male Piper voice from rhasspy/piper-voices.",
        license: "MIT",
        repository: "rhasspy/piper-voices",
        url: "https://huggingface.co/rhasspy/piper-voices/tree/main/ru/ru_RU/denis/medium",
        isDownloaded: false,
        isSelected: false,
      },
      {
        id: "ru_RU-irina-medium",
        name: "Irina",
        locale: "ru_RU",
        quality: "Medium",
        size: "~63 MB",
        description: "Russian female Piper voice from rhasspy/piper-voices.",
        license: "MIT",
        repository: "rhasspy/piper-voices",
        url: "https://huggingface.co/rhasspy/piper-voices/tree/main/ru/ru_RU/irina/medium",
        isDownloaded: false,
        isSelected: false,
      },
      {
        id: "ru_RU-ruslan-medium",
        name: "Ruslan",
        locale: "ru_RU",
        quality: "Medium",
        size: "~63 MB",
        description: "Russian male Piper voice from rhasspy/piper-voices.",
        license: "MIT",
        repository: "rhasspy/piper-voices",
        url: "https://huggingface.co/rhasspy/piper-voices/tree/main/ru/ru_RU/ruslan/medium",
        isDownloaded: false,
        isSelected: false,
      },
    ]);
  }
  return invoke<TtsVoiceInfo[]>("list_tts_voices");
}

export function listCustomTtsModels() {
  if (!hasTauriRuntime()) {
    return Promise.resolve<TtsCustomModelInfo[]>([]);
  }
  return invoke<TtsCustomModelInfo[]>("list_custom_tts_models");
}

export function downloadCustomTtsModel(input: {
  url: string;
  name?: string;
  license?: string;
  engine?: string;
}) {
  requireTauriRuntime();
  return invoke<TtsCustomModelInfo>("download_custom_tts_model", input);
}

export function deleteCustomTtsModel(modelId: string) {
  requireTauriRuntime();
  return invoke<void>("delete_custom_tts_model", { modelId });
}

export function setActiveCustomTtsModel(modelId: string) {
  requireTauriRuntime();
  return invoke<TtsStatus>("set_active_custom_tts_model", { modelId });
}

export function setCustomTtsModelVoice(modelId: string, voiceId: string) {
  requireTauriRuntime();
  return invoke<TtsCustomModelInfo>("set_custom_tts_model_voice", { modelId, voiceId });
}


export function downloadTtsVoice(voiceId: string) {
  requireTauriRuntime();
  return invoke<TtsStatus>("download_tts_voice", { voiceId });
}

export function downloadPiperEngine() {
  requireTauriRuntime();
  return invoke<TtsStatus>("download_piper_engine");
}

export function setTtsVoice(voiceId: string) {
  requireTauriRuntime();
  return invoke<TtsStatus>("set_tts_voice", { voiceId });
}

export function deleteTtsVoice(voiceId: string) {
  requireTauriRuntime();
  return invoke<TtsStatus>("delete_tts_voice", { voiceId });
}

export function openTtsModelsFolder() {
  requireTauriRuntime();
  return invoke<void>("open_tts_models_folder");
}

export function synthesizeSpeech(text: string) {
  requireTauriRuntime();
  return invoke<TtsSynthesisResult>("synthesize_speech", { text });
}

export function fileAssetUrl(path: string) {
  if (!hasTauriRuntime()) return path;
  return convertFileSrc(path);
}

export function deleteWhisperModel(modelId: string) {
  requireTauriRuntime();
  return invoke<ModelStatus>("delete_whisper_model", { modelId });
}

export function getHotkeySettings() {
  if (!hasTauriRuntime()) {
    return Promise.resolve<{ alwaysOn: boolean; hotkeyType: string; hotkeyDisplay: string; autoSubmit: boolean }>({
      alwaysOn: true,
      hotkeyType: "unassigned",
      hotkeyDisplay: "Unassigned",
      autoSubmit: false,
    });
  }
  return invoke<{ alwaysOn: boolean; hotkeyType: string; hotkeyDisplay: string; autoSubmit: boolean }>("get_hotkey_settings");
}

export function updateHotkeySettings(alwaysOn: boolean, hotkeyType: string, autoSubmit: boolean) {
  requireTauriRuntime();
  return invoke<void>("update_hotkey_settings", { alwaysOn, hotkeyType, autoSubmit });
}

export function startRecordingHotkey() {
  requireTauriRuntime();
  return invoke<void>("start_recording_hotkey");
}

export function cancelRecordingHotkey() {
  requireTauriRuntime();
  return invoke<void>("cancel_recording_hotkey");
}

export function injectText(text: string, autoSubmit: boolean) {
  requireTauriRuntime();
  return invoke<void>("inject_text", { text, autoSubmit });
}

export function startRecording() {
  requireTauriRuntime();
  return invoke<void>("start_recording");
}

export function stopRecording() {
  requireTauriRuntime();
  return invoke("stop_recording");
}

export function getRecordingState() {
  if (!hasTauriRuntime()) {
    return Promise.resolve(false);
  }
  return invoke<boolean>("get_recording_state");
}

export function transcribeAudio(language: Language) {
  requireTauriRuntime();
  return invoke<TranscriptResult>("transcribe_audio", { language });
}

export function saveTranscript(result: TranscriptResult) {
  requireTauriRuntime();
  return invoke<number>("save_transcript", {
    text: result.text,
    language: result.language,
    duration: result.durationSeconds,
    modelName: result.modelName,
  });
}

export function getHistory() {
  requireTauriRuntime();
  return invoke<HistoryItem[]>("get_history");
}

export function deleteHistoryItem(id: number) {
  requireTauriRuntime();
  return invoke<void>("delete_history_item", { id });
}

export function exportTxt(text: string) {
  requireTauriRuntime();
  return invoke<void>("export_txt", { text });
}

export function writeClipboard(text: string) {
  requireTauriRuntime();
  return invoke<void>("write_clipboard", { text });
}

export function errorMessage(error: unknown) {
  if (typeof error === "string") return error;
  if (error && typeof error === "object" && "message" in error) {
    return String((error as { message: unknown }).message);
  }
  return "Unexpected error.";
}

export function logMessage(message: string) {
  if (!hasTauriRuntime()) {
    console.log(message);
    return Promise.resolve();
  }
  return invoke<void>("log_message", { message });
}
