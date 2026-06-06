export type AppStatus = "ready" | "recording" | "processing" | "done" | "error";

export type Language = "auto" | "en" | "ru" | "uk" | "zh" | "es";

export interface TranscriptResult {
  text: string;
  language: Language | "unknown";
  durationSeconds: number;
  modelName: string;
}

export interface ModelStatus {
  exists: boolean;
  modelPath?: string;
  modelName?: string;
  backend: "cpu" | "metal" | "auto" | "cuda";
  engineExists: boolean;
  enginePath?: string;
  gpuEngineExists: boolean;
  gpuEnginePath?: string;
}

export interface WhisperModelInfo {
  id: string;
  name: string;
  fileName: string;
  size: string;
  description: string;
  url: string;
  localPath?: string;
  isDownloaded: boolean;
  isSelected: boolean;
}

export interface TtsStatus {
  engineExists: boolean;
  enginePath?: string;
  selectedVoiceId?: string;
  selectedVoiceName?: string;
  selectedVoiceDownloaded: boolean;
}

export interface TtsVoiceInfo {
  id: string;
  name: string;
  locale: string;
  quality: string;
  size: string;
  description: string;
  license: string;
  repository: string;
  url: string;
  localModelPath?: string;
  localConfigPath?: string;
  isDownloaded: boolean;
  isSelected: boolean;
}

export interface TtsCustomModelInfo {
  id: string;
  name: string;
  engine: string;
  license: string;
  url: string;
  localPath?: string;
  companionConfigPath?: string;
  isDownloaded: boolean;
  synthesisSupported: boolean;
  notes: string;
  voices: TtsCustomVoiceInfo[];
  selectedVoiceId?: string;
  isSelected: boolean;
}

export interface TtsCustomVoiceInfo {
  id: string;
  name: string;
  language: string;
  gender?: string;
}

export interface TtsSynthesisResult {
  audioPath: string;
  voiceName: string;
}

export interface AudioCaptureInfo {
  path: string;
  durationSeconds: number;
  sampleCount: number;
}

export interface HistoryItem {
  id: number;
  text: string;
  language: Language | "unknown";
  createdAt: string;
  durationSeconds: number;
  modelName: string;
}
