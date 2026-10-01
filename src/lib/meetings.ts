import { invoke } from "@tauri-apps/api/core";

export interface MeetingSpeaker { id: string; name: string; sampleStart: number; sampleEnd: number }
export interface MeetingTurn { start: number; end: number; speakerId: string | null }
export interface VoiceReference { speakerId: string; name: string; start: number; end: number }
export interface MeetingSegment { id: string; start: number; end: number; speakerId: string | null; text: string; needsReview: boolean; speakerLocked?: boolean }
export interface Meeting {
  id: string; title: string; sourceName: string; audioPath: string; durationSeconds: number;
  createdAt: string; language: string; modelName: string | null;
  speakers: MeetingSpeaker[]; turns: MeetingTurn[]; segments: MeetingSegment[];
  calibration?: { references: VoiceReference[]; calibratedAt: string; reviewSegments: number; engine?: string; alignment?: string; overlaps?: {start: number; end: number}[]; sourceMeetingId?: string } | null;
}
export interface MeetingProgress { stage: string; percentage: number; message: string }
export interface MeetingToolsStatus { pythonReady: boolean; ffmpegReady: boolean; diarizationReady: boolean; busy: boolean; progress: MeetingProgress | null }

function desktop<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  if (!window.__TAURI_INTERNALS__) return Promise.reject(new Error("Откройте Ferrofluid Voice на компьютере, чтобы обработать аудио."));
  return invoke<T>(command, args);
}
export const meetingToolsStatus = () => desktop<MeetingToolsStatus>("meeting_tools_status");
export const setupMeetingTools = () => desktop<void>("setup_meeting_tools");
export const cancelMeetingJob = () => desktop<void>("cancel_meeting_job");
export const importMeeting = (source: string) => desktop<Meeting>("import_meeting", { source });
export const analyzeMeeting = (id: string, speakerCount: number) => desktop<Meeting>("analyze_meeting", { id, speakerCount });
export const calibrateMeeting = (id: string, references: VoiceReference[]) => desktop<Meeting>("calibrate_meeting", { id, references });
export const refineMeeting = (id: string, language: string) => desktop<Meeting>("refine_meeting", { id, language });
export const transcribeMeeting = (id: string, language: string) => desktop<Meeting>("transcribe_meeting", { id, language });
export const listMeetings = () => desktop<Meeting[]>("list_meetings");
export const saveMeeting = (meeting: Meeting) => desktop<void>("save_meeting", { meeting });
export const exportMeetingFile = (text: string, extension: string) => desktop<boolean>("export_meeting_file", { text, extension });

export function timecode(seconds: number, milliseconds = false) {
  const ms = Math.max(0, Math.round(seconds * 1000));
  const h = Math.floor(ms / 3600000), m = Math.floor(ms / 60000) % 60, s = Math.floor(ms / 1000) % 60;
  const stamp = [h, m, s].map(n => String(n).padStart(2, "0")).join(":");
  return milliseconds ? `${stamp},${String(ms % 1000).padStart(3, "0")}` : stamp;
}
export function meetingExport(meeting: Meeting, format: "txt" | "srt" | "json") {
  const speaker = (id: string | null) => meeting.speakers.find(s => s.id === id)?.name || "Участник не определён";
  if (format === "json") return JSON.stringify({ ...meeting, segments: meeting.segments.map(s => ({ ...s, speakerName: speaker(s.speakerId) })) }, null, 2);
  if (format === "srt") return meeting.segments.map((s, i) => `${i + 1}\n${timecode(s.start, true)} --> ${timecode(s.end, true)}\n${speaker(s.speakerId)}: ${s.text}\n`).join("\n");
  return `${meeting.title}\n${meeting.sourceName} · ${timecode(meeting.durationSeconds)}\n\n` + meeting.segments.map(s => `[${timecode(s.start)}] ${speaker(s.speakerId)}${s.needsReview ? " [проверить]" : ""}: ${s.text}`).join("\n\n");
}
