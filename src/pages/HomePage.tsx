import { Clipboard, Download, Save, Trash2 } from "lucide-react";
import { useState } from "react";
import { GlassCard } from "../components/GlassCard";
import { RecordButton } from "../components/RecordButton";
import { StatusPill } from "../components/StatusPill";
import { TranscriptEditor } from "../components/TranscriptEditor";
import { errorMessage, exportTxt, saveTranscript, startRecording, stopRecording, transcribeAudio, writeClipboard } from "../lib/tauri";
import type { AppStatus, Language, ModelStatus, TranscriptResult } from "../lib/types";
import { formatDuration } from "../lib/format";

interface HomePageProps {
  modelStatus: ModelStatus;
}

export function HomePage({ modelStatus }: HomePageProps) {
  const [status, setStatus] = useState<AppStatus>("ready");
  const [language, setLanguage] = useState<Language>("auto");
  const [result, setResult] = useState<TranscriptResult | null>(null);
  const [text, setText] = useState("");
  const [message, setMessage] = useState("");

  async function handleRecordClick() {
    setMessage("");
    try {
      if (status === "recording") {
        setStatus("processing");
        await stopRecording();
        const nextResult = await transcribeAudio(language);
        setResult(nextResult);
        setText(nextResult.text);
        setStatus("done");
      } else {
        setStatus("recording");
        await startRecording();
      }
    } catch (error) {
      setStatus("error");
      setMessage(errorMessage(error));
    }
  }

  async function copyText() {
    try {
      await writeClipboard(text);
      setMessage("Copied to clipboard.");
    } catch (error) {
      setMessage(errorMessage(error));
    }
  }

  async function saveCurrentTranscript() {
    if (!text.trim()) return;
    try {
      await saveTranscript({
        text,
        language,
        durationSeconds: result?.durationSeconds ?? 0,
        modelName: result?.modelName ?? modelStatus.modelName ?? "unknown",
      });
      setMessage("Saved to history.");
    } catch (error) {
      setMessage(errorMessage(error));
    }
  }

  async function exportCurrentText() {
    if (!text.trim()) return;
    try {
      await exportTxt(text);
      setMessage("Exported.");
    } catch (error) {
      setMessage(errorMessage(error));
    }
  }

  function clearText() {
    setText("");
    setResult(null);
    setStatus("ready");
    setMessage("");
  }

  return (
    <div className="grid min-h-0 flex-1 gap-5 lg:grid-cols-[360px_minmax(0,1fr)]">
      <GlassCard className="flex flex-col justify-between p-6">
        <div>
          <div className="mb-5 flex items-center justify-between">
            <StatusPill status={status} />
            {result ? <span className="text-sm text-slate-600 dark:text-slate-300">{formatDuration(result.durationSeconds)}</span> : null}
          </div>

          <div className="grid place-items-center py-7">
            <RecordButton status={status} onClick={handleRecordClick} />
          </div>

          <div className="mt-3">
            <div className="mb-2 text-sm font-medium text-slate-700 dark:text-slate-200">Language</div>
            <div className="segmented">
              {[
                ["auto", "Auto"],
                ["ru", "Russian"],
                ["en", "English"],
              ].map(([value, label]) => (
                <button
                  key={value}
                  className={language === value ? "segmented-active" : ""}
                  onClick={() => setLanguage(value as Language)}
                  disabled={status === "recording" || status === "processing"}
                >
                  {label}
                </button>
              ))}
            </div>
          </div>
        </div>

        <div className="mt-7 space-y-3">
          <div className="rounded-xl border border-white/35 bg-white/35 p-3 text-sm leading-6 text-slate-600 shadow-insetGlass dark:border-white/10 dark:bg-white/8 dark:text-slate-300">
            Model: {modelStatus.modelName ?? "unknown"}
          </div>
          {message ? <div className={status === "error" ? "error-banner" : "success-banner"}>{message}</div> : null}
        </div>
      </GlassCard>

      <GlassCard className="flex min-h-[540px] flex-col p-6">
        <div className="mb-4 flex flex-wrap items-center justify-between gap-3">
          <div>
            <h1 className="text-3xl font-semibold tracking-normal">Transcript</h1>
            <p className="mt-1 text-sm text-slate-600 dark:text-slate-300">Record, review, copy, save, or export.</p>
          </div>
          <div className="flex gap-2">
            <button className="icon-button" onClick={copyText} disabled={!text.trim()} aria-label="Copy" title="Copy">
              <Clipboard className="h-4 w-4" />
            </button>
            <button className="icon-button" onClick={saveCurrentTranscript} disabled={!text.trim()} aria-label="Save to history" title="Save to history">
              <Save className="h-4 w-4" />
            </button>
            <button className="icon-button" onClick={exportCurrentText} disabled={!text.trim()} aria-label="Export TXT" title="Export TXT">
              <Download className="h-4 w-4" />
            </button>
            <button className="icon-button danger" onClick={clearText} disabled={!text.trim()} aria-label="Clear" title="Clear">
              <Trash2 className="h-4 w-4" />
            </button>
          </div>
        </div>
        <TranscriptEditor value={text} onChange={setText} />
      </GlassCard>
    </div>
  );
}
