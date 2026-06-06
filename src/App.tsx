import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { Sun, Moon } from "lucide-react";
import { motion, AnimatePresence } from "motion/react";
import { SettingsPage } from "./pages/SettingsPage";
import { LibraryPage } from "./pages/LibraryPage";
import { Widget } from "./components/Widget";
import { errorMessage, getModelStatus, openSettingsWindow } from "./lib/tauri";
import type { ModelStatus, Language } from "./lib/types";
import { useLocales } from "./lib/locales";

export default function App() {
  const [modelStatus, setModelStatus] = useState<ModelStatus | null>(null);
  const [bootError, setBootError] = useState("");
  const [theme, setTheme] = useState<"light" | "dark">(
    () => (localStorage.getItem("app_theme") as "light" | "dark") || "light"
  );
  const [language, setLanguage] = useState<Language>(
    () => (localStorage.getItem("transcribe_language") as Language) || "auto"
  );
  const view = new URLSearchParams(window.location.search).get("view");
  const isSettingsWindow = view === "settings";
  const isLibraryWindow = view === "library";

  useEffect(() => {
    const syncSettings = () => {
      const savedLang = localStorage.getItem("transcribe_language") as Language;
      if (savedLang) {
        setLanguage(savedLang);
      }
      const savedTheme = localStorage.getItem("app_theme") as "light" | "dark";
      if (savedTheme) {
        setTheme(savedTheme);
      }
    };

    syncSettings();

    window.addEventListener("focus", syncSettings);

    const handleStorage = (e: StorageEvent) => {
      if (e.key === "transcribe_language" && e.newValue) {
        setLanguage(e.newValue as Language);
      }
      if (e.key === "app_theme" && e.newValue) {
        setTheme(e.newValue as "light" | "dark");
      }
    };
    window.addEventListener("storage", handleStorage);

    return () => {
      window.removeEventListener("focus", syncSettings);
      window.removeEventListener("storage", handleStorage);
    };
  }, []);

  const t = useLocales(language);

  useEffect(() => {
    document.documentElement.classList.toggle("dark", theme === "dark");
  }, [theme]);

  useEffect(() => {
    void refreshModelStatus();
  }, [isSettingsWindow]);

  useEffect(() => {
    if (isSettingsWindow) return;

    const refresh = () => void refreshModelStatus();
    window.addEventListener("focus", refresh);
    document.addEventListener("visibilitychange", refresh);
    const interval = window.setInterval(refresh, 2500);

    return () => {
      window.removeEventListener("focus", refresh);
      document.removeEventListener("visibilitychange", refresh);
      window.clearInterval(interval);
    };
  }, [isSettingsWindow]);

  useEffect(() => {
    if (typeof window === "undefined" || !window.__TAURI_INTERNALS__) return;

    let unlisten: (() => void) | undefined;
    void listen<ModelStatus>("model-status-changed", (event) => {
      setModelStatus(event.payload);
    }).then((handler) => {
      unlisten = handler;
    });

    return () => unlisten?.();
  }, []);

  async function refreshModelStatus() {
    try {
      setBootError("");
      setModelStatus(await getModelStatus());
    } catch (error) {
      setBootError(errorMessage(error));
    }
  }

  const hasModel = modelStatus?.exists;

  async function openSettings() {
    try {
      setBootError("");
      await openSettingsWindow();
    } catch (error) {
      setBootError(errorMessage(error));
    }
  }

  if (isSettingsWindow || isLibraryWindow) {
    return (
      <main className="settings-stage text-slate-950 transition-colors dark:text-slate-50">
        <div className="settings-window settings-window-standalone">
          <div className="settings-window-header">
            <div>
              <div className="flex items-center gap-3">
                <div className="settings-window-title">
                  {isLibraryWindow ? "Ferrofluid Voice Library" : `Ferrofluid Voice ${t.settingsTitle}`}
                </div>
                {!isLibraryWindow ? (
                  <div className={`status-pill ${hasModel ? "status-ready" : "status-error"}`} style={{ minHeight: "1.65rem", height: "1.65rem", fontSize: "0.74rem", padding: "0 0.65rem" }}>
                    {hasModel ? t.modelFound : t.modelMissing}
                  </div>
                ) : null}
              </div>
              <div className="settings-window-subtitle">
                {isLibraryWindow ? "История распознавания и озвучивание текста" : hasModel ? t.settingsSubtitle : t.hintSelectModel}
              </div>
            </div>
            <div className="flex gap-2">
              <AnimatePresence mode="wait" initial={false}>
                <motion.button
                  key={theme}
                  initial={{ opacity: 0, scale: 0.8, rotate: -45 }}
                  animate={{ opacity: 1, scale: 1, rotate: 0 }}
                  exit={{ opacity: 0, scale: 0.8, rotate: 45 }}
                  transition={{ duration: 0.2, ease: "easeInOut" }}
                  className="widget-action-btn"
                  onClick={() => {
                    const newTheme = theme === "light" ? "dark" : "light";
                    setTheme(newTheme);
                    localStorage.setItem("app_theme", newTheme);
                  }}
                  title={theme === "light" ? t.themeDark : t.themeLight}
                  aria-label={theme === "light" ? t.themeDark : t.themeLight}
                  style={{
                    width: "36px",
                    height: "36px",
                    borderRadius: "50%",
                    display: "flex",
                    alignItems: "center",
                    justifyContent: "center",
                  }}
                >
                  {theme === "light" ? (
                    <Sun className="h-4.5 w-4.5 text-amber-500 fill-amber-500/10" />
                  ) : (
                    <Moon className="h-4.5 w-4.5 text-indigo-300 fill-indigo-300/10" />
                  )}
                </motion.button>
              </AnimatePresence>
            </div>
          </div>
          {isLibraryWindow ? (
            <LibraryPage />
          ) : !modelStatus ? (
            <div className="glass-panel grid min-h-[420px] place-items-center text-slate-600 dark:text-slate-300">{t.loading}</div>
          ) : (
            <SettingsPage modelStatus={modelStatus} onModelStatusChange={setModelStatus} language={language} onLanguageChange={setLanguage} />
          )}
          {bootError ? <div className="widget-floating-error">{bootError}</div> : null}
        </div>
      </main>
    );
  }

  return (
    <main className="widget-stage text-slate-950 transition-colors dark:text-slate-50">
      <Widget language={language} modelStatus={modelStatus} onOpenSettings={openSettings} />
      {bootError ? <div className="widget-floating-error">{bootError}</div> : null}
    </main>
  );
}
