import { invoke } from "@tauri-apps/api/core";

export interface QwenVoice { id: string; name: string; referenceText: string }
export interface QwenProgress { message: string; percentage: number; error?: boolean; audioPath?: string; voiceName?: string }
export interface QwenStatus { ready: boolean; busy: boolean; progress?: QwenProgress; voices: QwenVoice[]; gpu?: string }
export const qwenStatus = () => invoke<QwenStatus>("qwen_status");
export const setupQwen = () => invoke<void>("setup_qwen");
export const cancelQwen = () => invoke<void>("cancel_qwen");
export const synthesizeQwen = (text: string, voiceId: string, style: string) => invoke<{ audioPath: string; voiceName: string }>("synthesize_qwen", { text, voiceId, style });
export const importQwenVoice = (source: string, name: string, referenceText: string) => invoke<QwenVoice>("import_qwen_voice", { source, name, referenceText });
export const deleteQwenVoice = (voiceId: string) => invoke<void>("delete_qwen_voice", { voiceId });
export const exportQwenAudio = (source: string, destination: string) => invoke<void>("export_qwen_audio", { source, destination });

export const qwenSpeakers = [
  { id: "Ryan", name: "Ryan", description: "Мужской · выразительный" },
  { id: "Aiden", name: "Aiden", description: "Мужской · мягкий" },
  { id: "Uncle_Fu", name: "Uncle Fu", description: "Мужской · низкий, спокойный" },
  { id: "Dylan", name: "Dylan", description: "Мужской · молодой" },
  { id: "Eric", name: "Eric", description: "Мужской · чуть хриплый" },
  { id: "Vivian", name: "Vivian", description: "Женский · яркий" },
  { id: "Serena", name: "Serena", description: "Женский · тёплый" },
  { id: "Ono_Anna", name: "Anna", description: "Женский · лёгкий" },
  { id: "Sohee", name: "Sohee", description: "Женский · эмоциональный" },
];
