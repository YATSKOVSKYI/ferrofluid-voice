import { CheckCircle2, CircleAlert, Loader2, Mic, Sparkles } from "lucide-react";
import type { AppStatus } from "../lib/types";

interface StatusPillProps {
  status: AppStatus;
  label?: string;
}

export function StatusPill({ status, label }: StatusPillProps) {
  const Icon =
    status === "recording" ? Mic : status === "processing" ? Loader2 : status === "error" ? CircleAlert : status === "done" ? CheckCircle2 : Sparkles;

  return (
    <div className={`status-pill status-${status}`}>
      <Icon className={`h-4 w-4 ${status === "processing" ? "animate-spin" : ""}`} />
      <span>{label ?? statusLabel(status)}</span>
    </div>
  );
}

function statusLabel(status: AppStatus) {
  if (status === "recording") return "Recording";
  if (status === "processing") return "Processing";
  if (status === "done") return "Done";
  if (status === "error") return "Error";
  return "Ready";
}
