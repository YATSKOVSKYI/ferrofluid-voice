import { Mic, Square } from "lucide-react";
import type { AppStatus } from "../lib/types";

interface RecordButtonProps {
  status: AppStatus;
  onClick: () => void;
}

export function RecordButton({ status, onClick }: RecordButtonProps) {
  const active = status === "recording";
  const disabled = status === "processing";
  return (
    <button
      className={`record-button ${active ? "record-button-active" : ""}`}
      onClick={onClick}
      disabled={disabled}
      aria-label={active ? "Stop recording" : "Start recording"}
      title={active ? "Stop recording" : "Start recording"}
    >
      {active ? <Square className="h-10 w-10 fill-current" /> : <Mic className="h-11 w-11" />}
    </button>
  );
}
