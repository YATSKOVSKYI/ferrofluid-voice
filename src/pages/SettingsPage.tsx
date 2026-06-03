import { SettingsPanel } from "../components/SettingsPanel";
import type { ModelStatus, Language } from "../lib/types";

interface SettingsPageProps {
  modelStatus: ModelStatus;
  onModelStatusChange: (status: ModelStatus) => void;
  language: Language;
  onLanguageChange: (lang: Language) => void;
}

export function SettingsPage({ modelStatus, onModelStatusChange, language, onLanguageChange }: SettingsPageProps) {
  return (
    <div className="flex flex-1 flex-col gap-5">
      <SettingsPanel
        modelStatus={modelStatus}
        onModelStatusChange={onModelStatusChange}
        language={language}
        onLanguageChange={onLanguageChange}
      />
    </div>
  );
}
