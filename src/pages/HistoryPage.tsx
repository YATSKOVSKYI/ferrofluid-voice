import { useEffect, useState } from "react";
import { GlassCard } from "../components/GlassCard";
import { HistoryList } from "../components/HistoryList";
import { deleteHistoryItem, errorMessage, getHistory, writeClipboard } from "../lib/tauri";
import type { HistoryItem } from "../lib/types";

export function HistoryPage() {
  const [items, setItems] = useState<HistoryItem[]>([]);
  const [message, setMessage] = useState("");

  useEffect(() => {
    load();
  }, []);

  async function load() {
    try {
      setItems(await getHistory());
      setMessage("");
    } catch (error) {
      setMessage(errorMessage(error));
    }
  }

  async function copy(text: string) {
    try {
      await writeClipboard(text);
      setMessage("Copied to clipboard.");
    } catch (error) {
      setMessage(errorMessage(error));
    }
  }

  async function remove(id: number) {
    try {
      await deleteHistoryItem(id);
      await load();
    } catch (error) {
      setMessage(errorMessage(error));
    }
  }

  return (
    <GlassCard className="flex flex-1 flex-col p-6">
      <div className="mb-5 flex items-start justify-between">
        <div>
          <h1 className="text-3xl font-semibold tracking-normal">History</h1>
          <p className="mt-1 text-sm text-slate-600 dark:text-slate-300">Saved dictation results</p>
        </div>
        <button className="secondary-button" onClick={load}>Refresh</button>
      </div>
      {message ? <div className="success-banner mb-4">{message}</div> : null}
      <HistoryList items={items} onCopy={copy} onDelete={remove} />
    </GlassCard>
  );
}
