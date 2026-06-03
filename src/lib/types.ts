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
