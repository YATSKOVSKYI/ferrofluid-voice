import { open } from "@tauri-apps/plugin-dialog";
import { FolderOpen, HardDrive, Info } from "lucide-react";
import { useState } from "react";
import { errorMessage, openModelsFolder, setModelPath } from "../lib/tauri";
import type { ModelStatus } from "../lib/types";
import { GlassCard } from "./GlassCard";

interface ModelSetupProps {
  onModelReady: (status: ModelStatus) => void;
}

export function ModelSetup({ onModelReady }: ModelSetupProps) {
  const [error, setError] = useState("");
  const [busy, setBusy] = useState(false);

  async function chooseModel() {
    setBusy(true);
    setError("");
    try {
      const selected = await open({
        multiple: false,
        filters: [{ name: "Whisper model", extensions: ["bin"] }],
      });
      if (typeof selected === "string") {
        onModelReady(await setModelPath(selected));
      }
    } catch (error) {
      setError(errorMessage(error));
    } finally {
      setBusy(false);
    }
  }

  async function revealFolder() {
    try {
      await openModelsFolder();
    } catch (error) {
      setError(errorMessage(error));
    }
  }

  return (
    <GlassCard className="flex flex-1 items-center justify-center p-8">
      <div className="w-full max-w-2xl">
        <div className="mb-7 flex h-14 w-14 items-center justify-center rounded-2xl bg-white/55 shadow-insetGlass dark:bg-white/12">
          <HardDrive className="h-7 w-7" />
        </div>
        <h1 className="text-4xl font-semibold tracking-normal">Set up a local model</h1>
        <p className="mt-3 max-w-xl text-base leading-7 text-slate-600 dark:text-slate-300">
          Choose a whisper.cpp GGML model once. Ferrofluid Voice stores the path and reuses it on future launches.
        </p>

        <div className="mt-8 grid gap-3 sm:grid-cols-2">
          <button className="primary-button h-12" onClick={chooseModel} disabled={busy}>
            <FolderOpen className="h-5 w-5" />
            Choose model
          </button>
          <button className="secondary-button h-12" onClick={revealFolder}>
            Open models folder
          </button>
        </div>

        <div className="mt-6 flex gap-3 rounded-xl border border-white/45 bg-white/38 p-4 text-sm leading-6 text-slate-600 shadow-insetGlass dark:border-white/10 dark:bg-white/8 dark:text-slate-300">
          <Info className="mt-0.5 h-4 w-4 shrink-0" />
          <span>
            Recommended MVP model: small or medium GGML `.bin`. The Whisper executable is expected as a bundled
            `whisper-cli` sidecar or via `FERROFLUID_WHISPER_BIN` during development.
          </span>
        </div>

        {error ? <div className="error-banner mt-5">{error}</div> : null}
      </div>
    </GlassCard>
  );
}
