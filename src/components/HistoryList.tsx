import { Copy, Trash2 } from "lucide-react";
import type { HistoryItem } from "../lib/types";
import { formatDate, formatDuration, languageLabel } from "../lib/format";

interface HistoryListProps {
  items: HistoryItem[];
  onCopy: (text: string) => void;
  onDelete: (id: number) => void;
}

export function HistoryList({ items, onCopy, onDelete }: HistoryListProps) {
  if (items.length === 0) {
    return <div className="empty-state">No saved transcripts yet.</div>;
  }

  return (
    <div className="space-y-3">
      {items.map((item) => (
        <article key={item.id} className="history-item">
          <div className="min-w-0 flex-1">
            <div className="mb-2 flex flex-wrap items-center gap-2 text-xs text-slate-500 dark:text-slate-400">
              <span>{formatDate(item.createdAt)}</span>
              <span>{languageLabel(item.language)}</span>
              <span>{formatDuration(item.durationSeconds)}</span>
              <span>{item.modelName}</span>
            </div>
            <p className="line-clamp-4 whitespace-pre-wrap text-sm leading-6 text-slate-800 dark:text-slate-100">{item.text}</p>
          </div>
          <div className="flex shrink-0 gap-2">
            <button className="icon-button" onClick={() => onCopy(item.text)} aria-label="Copy transcript" title="Copy transcript">
              <Copy className="h-4 w-4" />
            </button>
            <button className="icon-button danger" onClick={() => onDelete(item.id)} aria-label="Delete transcript" title="Delete transcript">
              <Trash2 className="h-4 w-4" />
            </button>
          </div>
        </article>
      ))}
    </div>
  );
}
