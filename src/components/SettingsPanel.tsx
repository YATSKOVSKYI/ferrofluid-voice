import { CheckCircle2, Download, HardDrive, Keyboard, Loader2, Trash2, X, FolderOpen, Settings, Volume2 } from "lucide-react";
import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import {
  cancelDownload,
  deleteWhisperModel,
  downloadWhisperModel,
  getDownloadProgress,
  getHotkeySettings,
  updateHotkeySettings,
  startRecordingHotkey,
  cancelRecordingHotkey,
  errorMessage,
  listWhisperModels,
  setModelPath,
  openModelsFolder,
} from "../lib/tauri";
import type { ModelStatus, WhisperModelInfo, Language } from "../lib/types";
import { useLocales } from "../lib/locales";
import { TtsModelsPanel } from "./TtsModelsPanel";

interface SettingsPanelProps {
  modelStatus: ModelStatus;
  onModelStatusChange: (status: ModelStatus) => void;
  language: Language;
  onLanguageChange: (lang: Language) => void;
}

export function SettingsPanel({
  modelStatus,
  onModelStatusChange,
  language,
  onLanguageChange,
}: SettingsPanelProps) {
  const [quality, setQuality] = useState("balanced");
  const [backend, setBackend] = useState<ModelStatus["backend"]>(modelStatus.backend);
  const [models, setModels] = useState<WhisperModelInfo[]>([]);
  const [loadingModels, setLoadingModels] = useState(true);
  const [activeDownload, setActiveDownload] = useState("");
  const [downloadProgress, setDownloadProgress] = useState<
    Record<string, { downloadedBytes: number; totalBytes: number; percentage: number }>
  >({});
  const [error, setError] = useState("");
  const [alwaysOn, setAlwaysOn] = useState(true);
  const [hotkeyType, setHotkeyType] = useState("mouse_middle");
  const [autoSubmit, setAutoSubmit] = useState(false);
  const [hotkeyDisplay, setHotkeyDisplay] = useState("Middle Click");
  const [recordStatus, setRecordStatus] = useState<"idle" | "recording" | "success" | "cancelled">(
    "idle"
  );
  const [tempRecordedName, setTempRecordedName] = useState("");
  const [activeTab, setActiveTab] = useState<"models" | "tts" | "hotkeys" | "general">("models");

  const t = useLocales(language);

  const getModelDesc = (id: string, fallback: string) => {
    switch (id) {
      case "tiny": return t.modelDescTiny;
      case "base": return t.modelDescBase;
      case "small": return t.modelDescSmall;
      case "medium": return t.modelDescMedium;
      case "large-v3-turbo": return t.modelDescLargeV3Turbo;
      case "large-v3": return t.modelDescLargeV3;
      default: return fallback;
    }
  };

  useEffect(() => {
    if (typeof window === "undefined" || !window.__TAURI_INTERNALS__) return;

    let unlisten: (() => void) | undefined;
    void listen<any>("download-progress", (event) => {
      const payload = event.payload;
      const modelId = payload.modelId || payload.model_id;
      const downloadedBytes = payload.downloadedBytes || payload.downloaded_bytes || 0;
      const totalBytes = payload.totalBytes || payload.total_bytes || 0;
      const percentage = payload.percentage !== undefined ? payload.percentage : 0;

      if (modelId) {
        setDownloadProgress((prev) => ({
          ...prev,
          [modelId]: {
            downloadedBytes,
            totalBytes,
            percentage: Math.round(percentage),
          },
        }));
      }
    }).then((handler) => {
      unlisten = handler;
    });

    return () => unlisten?.();
  }, []);

  // Polling fallback to guarantee real-time updates even if Tauri events fail
  useEffect(() => {
    if (!activeDownload) return;

    const interval = window.setInterval(async () => {
      try {
        const progress = await getDownloadProgress(activeDownload);
        if (progress) {
          setDownloadProgress((prev) => ({
            ...prev,
            [activeDownload]: {
              downloadedBytes: progress.downloadedBytes,
              totalBytes: progress.totalBytes,
              percentage: Math.round(progress.percentage),
            },
          }));
        }
      } catch (e) {
        console.error("Error polling download progress:", e);
      }
    }, 250);

    return () => window.clearInterval(interval);
  }, [activeDownload]);

  useEffect(() => {
    async function loadHotkeySettings() {
      try {
        const hSettings = await getHotkeySettings();
        setAlwaysOn(hSettings.alwaysOn);
        setHotkeyType(hSettings.hotkeyType);
        setHotkeyDisplay(hSettings.hotkeyDisplay);
        setAutoSubmit(hSettings.autoSubmit);
      } catch (e) {
        console.error("Error loading hotkey settings:", e);
      }
    }
    void loadHotkeySettings();
  }, []);

  useEffect(() => {
    if (typeof window === "undefined" || !window.__TAURI_INTERNALS__) return;

    let unlisten: (() => void) | undefined;
    void listen<any>("hotkey-recorded", (event) => {
      const recordedType = event.payload.hotkeyType || event.payload.hotkey_type;
      const recordedName = event.payload.displayName || event.payload.display_name;
      if (recordedType && recordedName) {
        setHotkeyType(recordedType);
        setHotkeyDisplay(recordedName);
        setTempRecordedName(recordedName);
        setRecordStatus("success");
        void updateHotkeySettings(alwaysOn, recordedType, autoSubmit);

        // Revert back to idle status after 2 seconds of visual success confirmation
        setTimeout(() => {
          setRecordStatus("idle");
        }, 2000);
      }
    }).then((handler) => {
      unlisten = handler;
    });

    return () => unlisten?.();
  }, [alwaysOn, autoSubmit]);

  const handleCancelRecording = async () => {
    try {
      await cancelRecordingHotkey();
    } catch (err) {
      console.error("Error cancelling hotkey recording:", err);
    }
    setRecordStatus("cancelled");
    setTimeout(() => {
      setRecordStatus("idle");
    }, 1500);
  };

  useEffect(() => {
    if (recordStatus !== "recording") return;

    const handleKeyDown = async (e: KeyboardEvent) => {
      e.preventDefault();
      e.stopPropagation();

      const vkCode = e.keyCode;
      if (e.key === "Escape" || vkCode === 27) {
        await handleCancelRecording();
        return;
      }

      // Capture standard keyboard key as hotkey
      const recordedType = `key_${vkCode}`;
      const recordedName = getJsKeyDisplayName(vkCode);

      try {
        await cancelRecordingHotkey();
      } catch (err) {
        console.error("Error cancelling backend hotkey recording:", err);
      }

      setHotkeyType(recordedType);
      setHotkeyDisplay(recordedName);
      setTempRecordedName(recordedName);
      setRecordStatus("success");
      void updateHotkeySettings(alwaysOn, recordedType, autoSubmit);

      // Revert back to idle status after 2 seconds of visual success confirmation
      setTimeout(() => {
        setRecordStatus("idle");
      }, 2000);
    };

    const handlePointerDown = async (e: PointerEvent) => {
      e.preventDefault();
      e.stopPropagation();

      if (e.button === 0 || e.button === 2) {
        setError(t.leftRightMouseWarning);
        await handleCancelRecording();
        return;
      }

      let recordedType = "";
      let recordedName = "";

      if (e.button === 1) {
        recordedType = "mouse_middle";
        recordedName = language === "ru" ? "Клик колесиком (Middle Click)" : "Middle Click";
      } else if (e.button === 3) {
        recordedType = "mouse_x1";
        recordedName = language === "ru" ? "Боковая кнопка 4 (X1)" : "Side Button 4 (X1)";
      } else if (e.button === 4) {
        recordedType = "mouse_x2";
        recordedName = language === "ru" ? "Боковая кнопка 5 (X2)" : "Side Button 5 (X2)";
      } else {
        return; // Ignore other buttons
      }

      try {
        await cancelRecordingHotkey();
      } catch (err) {
        console.error("Error cancelling backend hotkey recording:", err);
      }

      setHotkeyType(recordedType);
      setHotkeyDisplay(recordedName);
      setTempRecordedName(recordedName);
      setRecordStatus("success");
      void updateHotkeySettings(alwaysOn, recordedType, autoSubmit);

      setTimeout(() => {
        setRecordStatus("idle");
      }, 2000);
    };

    const preventContextMenu = (e: MouseEvent) => {
      e.preventDefault();
      e.stopPropagation();
    };

    window.addEventListener("keydown", handleKeyDown, true);
    window.addEventListener("pointerdown", handlePointerDown, true);
    window.addEventListener("contextmenu", preventContextMenu, true);

    return () => {
      window.removeEventListener("keydown", handleKeyDown, true);
      window.removeEventListener("pointerdown", handlePointerDown, true);
      window.removeEventListener("contextmenu", preventContextMenu, true);
    };
  }, [recordStatus, alwaysOn, autoSubmit, language, t.leftRightMouseWarning]);

  async function handleHotkeySettingsChange(
    newAlwaysOn: boolean,
    newHotkey: string,
    newSubmit: boolean
  ) {
    setAlwaysOn(newAlwaysOn);
    setHotkeyType(newHotkey);
    setAutoSubmit(newSubmit);
    try {
      await updateHotkeySettings(newAlwaysOn, newHotkey, newSubmit);
    } catch (e) {
      setError(errorMessage(e));
    }
  }

  useEffect(() => {
    void refreshModels();
  }, [modelStatus.modelPath]);

  async function refreshModels() {
    try {
      setLoadingModels(true);
      setError("");
      setModels(await listWhisperModels());
    } catch (error) {
      setError(errorMessage(error));
    } finally {
      setLoadingModels(false);
    }
  }

  async function downloadModel(modelId: string) {
    try {
      setError("");
      setActiveDownload(modelId);
      onModelStatusChange(await downloadWhisperModel(modelId));
      setModels(await listWhisperModels());
    } catch (error) {
      const msg = errorMessage(error);
      if (!msg.toLowerCase().includes("cancelled") && !msg.toLowerCase().includes("cancel")) {
        setError(msg);
      }
    } finally {
      setActiveDownload("");
      setDownloadProgress((prev) => {
        const next = { ...prev };
        delete next[modelId];
        return next;
      });
    }
  }

  async function handleCancelDownload() {
    try {
      await cancelDownload();
    } catch (error) {
      setError(errorMessage(error));
    }
  }

  async function handleDeleteModel(modelId: string) {
    try {
      setError("");
      onModelStatusChange(await deleteWhisperModel(modelId));
      setModels(await listWhisperModels());
    } catch (error) {
      setError(errorMessage(error));
    }
  }

  async function useDownloadedModel(model: WhisperModelInfo) {
    if (!model.localPath) return;

    try {
      setError("");
      onModelStatusChange(await setModelPath(model.localPath));
      setModels(await listWhisperModels());
    } catch (error) {
      setError(errorMessage(error));
    }
  }

  async function handleOpenFolder() {
    try {
      await openModelsFolder();
    } catch (e) {
      setError(errorMessage(e));
    }
  }

  return (
    <div className="settings-panel-content flex-1 min-h-0 flex flex-col pt-2">
      <div className="settings-tabs-container">
        {/* Left Sidebar Tab Navigation */}
        <aside className="settings-sidebar">
          <button
            className={`sidebar-tab-btn ${activeTab === "models" ? "active" : ""}`}
            onClick={() => setActiveTab("models")}
          >
            <HardDrive className="h-4 w-4" />
            <span>{t.tabModels}</span>
          </button>
          <button
            className={`sidebar-tab-btn ${activeTab === "tts" ? "active" : ""}`}
            onClick={() => setActiveTab("tts")}
          >
            <Volume2 className="h-4 w-4" />
            <span>TTS модели</span>
          </button>
          <button
            className={`sidebar-tab-btn ${activeTab === "hotkeys" ? "active" : ""}`}
            onClick={() => setActiveTab("hotkeys")}
          >
            <Keyboard className="h-4 w-4" />
            <span>{t.tabHotkeys}</span>
          </button>
          <button
            className={`sidebar-tab-btn ${activeTab === "general" ? "active" : ""}`}
            onClick={() => setActiveTab("general")}
          >
            <Settings className="h-4 w-4" />
            <span>{t.tabGeneral}</span>
          </button>
        </aside>

        {/* Right Active Tab Content */}
        <main className="settings-tab-content">
          {activeTab === "models" && (
            <div className="flex flex-col gap-6">
              {/* Active Model */}
              <section className="settings-section">
                <div className="settings-section-heading">
                  <HardDrive className="h-4 w-4" />
                  <span>{t.localModelSec}</span>
                </div>

                <div
                  className="setting-row-flex"
                  style={{ display: "flex", gap: "10px", alignItems: "center" }}
                >
                  <div style={{ flex: 1 }}>
                    <input
                      value={
                        modelStatus.modelName ||
                        (modelStatus.modelPath ? modelStatus.modelPath.split(/[\\/]/).pop() : "") ||
                        "None"
                      }
                      readOnly
                      className="glass-input"
                      placeholder="No model selected"
                    />
                  </div>
                  <button
                    className="secondary-button"
                    style={{
                      height: "2.75rem",
                      minHeight: "2.75rem",
                      padding: "0 1rem",
                      borderRadius: "14px",
                      display: "flex",
                      alignItems: "center",
                      gap: "8px",
                      whiteSpace: "nowrap",
                    }}
                    onClick={handleOpenFolder}
                  >
                    <FolderOpen className="h-4.5 w-4.5" />
                    <span>{t.openFolderBtn}</span>
                  </button>
                </div>
              </section>

              {/* Models Download Section */}
              <section className="settings-section">
                <div className="settings-section-heading">
                  <Download className="h-4 w-4" />
                  <span>{t.downloadSec}</span>
                </div>

                <div className="model-list">
                  {loadingModels ? (
                    <div className="model-row model-row-muted">
                      <Loader2 className="h-4 w-4 animate-spin" />
                      {t.loadingModels}
                    </div>
                  ) : (
                    models.map((model) => {
                      const downloading = activeDownload === model.id;
                      const percentage = downloadProgress[model.id]?.percentage || 0;
                      return (
                        <div className="model-row-container" key={model.id}>
                          <div className="model-row">
                            <div className="model-row-main">
                              <div className="model-row-title">
                                <span>{model.name}</span>
                                <span>{model.size}</span>
                                {model.isSelected ? (
                                  <CheckCircle2 className="h-4 w-4 text-emerald-600 dark:text-emerald-400" />
                                ) : null}
                              </div>
                              <div className="model-row-description">{getModelDesc(model.id, model.description)}</div>
                              <div className="model-row-file">{model.fileName}</div>
                            </div>
                            {model.isDownloaded ? (
                              <div className="flex gap-2">
                                <button
                                  className="secondary-button"
                                  onClick={() => useDownloadedModel(model)}
                                  disabled={model.isSelected}
                                  style={{
                                    height: "2.1rem",
                                    minHeight: "2.1rem",
                                    padding: "0 0.8rem",
                                    borderRadius: "8px",
                                    fontSize: "0.78rem",
                                  }}
                                >
                                  {model.isSelected ? t.btnSelected : t.btnUse}
                                </button>
                                <button
                                  className="secondary-button hover:bg-rose-50 dark:hover:bg-rose-950/20 hover:text-rose-600 hover:border-rose-300 transition-colors"
                                  onClick={() => handleDeleteModel(model.id)}
                                  disabled={model.isSelected}
                                  title={t.btnDeleteTooltip}
                                  style={{
                                    height: "2.1rem",
                                    minHeight: "2.1rem",
                                    padding: "0 0.8rem",
                                    borderRadius: "8px",
                                    fontSize: "0.78rem",
                                  }}
                                >
                                  <Trash2 className="h-4 w-4" />
                                </button>
                              </div>
                            ) : (
                              <button
                                className={
                                  downloading
                                    ? "secondary-button border-rose-300 text-rose-600 hover:bg-rose-50 dark:hover:bg-rose-950/20"
                                    : "primary-button"
                                }
                                onClick={() =>
                                  downloading ? handleCancelDownload() : downloadModel(model.id)
                                }
                                disabled={Boolean(activeDownload) && !downloading}
                                style={{
                                  minWidth: "125px",
                                  height: "2.1rem",
                                  minHeight: "2.1rem",
                                  padding: "0 0.8rem",
                                  borderRadius: "8px",
                                  fontSize: "0.78rem",
                                }}
                              >
                                {downloading ? (
                                  <X className="h-4 w-4" />
                                ) : (
                                  <Download className="h-4 w-4" />
                                )}
                                <span style={{ marginLeft: "4px" }}>
                                  {downloading ? t.btnCancel : t.btnDownload}
                                </span>
                              </button>
                            )}
                          </div>

                          {/* Beautiful Sleek Download Progress Bar */}
                          {downloading && (
                            <div className="download-progress-container">
                              <div
                                style={{
                                  display: "flex",
                                  justifyContent: "space-between",
                                  fontSize: "0.72rem",
                                  fontWeight: 650,
                                  marginBottom: "4px",
                                  color: "var(--text-secondary)",
                                }}
                              >
                                <span>{t.loading}...</span>
                                <span>
                                  {downloadProgress[model.id]
                                    ? `${formatBytes(downloadProgress[model.id].downloadedBytes)} / ${formatBytes(downloadProgress[model.id].totalBytes)} (${percentage}%)`
                                    : `0.0 MB / ${model.size} (0%)`}
                                </span>
                              </div>
                              <div className="download-progress-bar">
                                <div
                                  className="download-progress-bar-fill"
                                  style={{ width: `${percentage}%` }}
                                ></div>
                              </div>
                            </div>
                          )}
                        </div>
                      );
                    })
                  )}
                </div>
              </section>
            </div>
          )}

          {activeTab === "tts" && <TtsModelsPanel onError={setError} />}

          {activeTab === "hotkeys" && (
            <div className="flex flex-col gap-6">
              <section className="settings-section">
                <div className="settings-section-heading">
                  <Keyboard className="h-4 w-4" />
                  <span>{t.hotkeySec}</span>
                </div>

                <div className="settings-grid">
                  <label className="setting-row">
                    <span>{t.opMode}</span>
                    <select
                      value={alwaysOn ? "always" : "hold"}
                      onChange={(event) => {
                        const val = event.target.value === "always";
                        void handleHotkeySettingsChange(val, hotkeyType, autoSubmit);
                      }}
                      className="glass-input"
                    >
                      <option value="always">{t.opModeAlways}</option>
                      <option value="hold">{t.opModeHold}</option>
                    </select>
                  </label>

                  <div
                    className="setting-row"
                    style={{
                      display: "flex",
                      flexDirection: "row",
                      alignItems: "center",
                      justifyContent: "space-between",
                    }}
                  >
                    <span>{t.activationHotkey}</span>
                    <div
                      className="flex gap-2 items-center"
                      style={{ display: "flex", flexDirection: "row", gap: "10px", alignItems: "center" }}
                    >
                      {recordStatus === "recording" && (
                        <button
                          className="primary-button animate-pulse"
                          style={{
                            background: "linear-gradient(135deg, #f43f5e, #e11d48)",
                            borderColor: "#e11d48",
                            color: "white",
                            minWidth: "220px",
                            minHeight: "2.1rem",
                            height: "2.1rem",
                            fontSize: "0.78rem",
                            fontWeight: 600,
                            borderRadius: "8px",
                            display: "flex",
                            alignItems: "center",
                            justifyContent: "center",
                            gap: "6px",
                          }}
                          onClick={handleCancelRecording}
                        >
                          <Loader2 className="h-3.5 w-3.5 animate-spin" />
                          <span>{t.hotkeyPressPrompt}</span>
                        </button>
                      )}

                      {recordStatus === "success" && (
                        <div
                          className="primary-button"
                          style={{
                            background: "linear-gradient(135deg, #10b981, #059669)",
                            borderColor: "#059669",
                            color: "white",
                            minWidth: "220px",
                            minHeight: "2.1rem",
                            height: "2.1rem",
                            fontSize: "0.78rem",
                            fontWeight: 600,
                            borderRadius: "8px",
                            display: "flex",
                            alignItems: "center",
                            justifyContent: "center",
                            gap: "6px",
                            boxShadow: "0 0 10px rgba(16, 185, 129, 0.4)",
                          }}
                        >
                          <CheckCircle2 className="h-4 w-4" />
                          <span>{t.hotkeyRecorded.replace("{name}", tempRecordedName)}</span>
                        </div>
                      )}

                      {recordStatus === "cancelled" && (
                        <div
                          className="primary-button"
                          style={{
                            background: "linear-gradient(135deg, #f59e0b, #d97706)",
                            borderColor: "#d97706",
                            color: "white",
                            minWidth: "220px",
                            minHeight: "2.1rem",
                            height: "2.1rem",
                            fontSize: "0.78rem",
                            fontWeight: 600,
                            borderRadius: "8px",
                            display: "flex",
                            alignItems: "center",
                            justifyContent: "center",
                            gap: "6px",
                          }}
                        >
                          <span>{t.hotkeyCancelled}</span>
                        </div>
                      )}

                      {recordStatus === "idle" && (
                        <>
                          <div
                            className="glass-input"
                            style={{
                              minWidth: "150px",
                              textAlign: "center",
                              fontWeight: 700,
                              background: "rgba(255,255,255,0.05)",
                              opacity: 1,
                              display: "flex",
                              alignItems: "center",
                              justifyContent: "center",
                              minHeight: "2.1rem",
                              height: "2.1rem",
                              borderRadius: "8px",
                              border: "1px solid var(--glass-border)",
                              padding: "0 12px",
                              fontSize: "0.78rem",
                              color: "var(--text-primary)",
                            }}
                          >
                            {hotkeyDisplay === "Unassigned" ? t.hotkeyUnassigned : hotkeyDisplay}
                          </div>
                          <button
                            className="secondary-button"
                            style={{
                              minHeight: "2.1rem",
                              height: "2.1rem",
                              fontSize: "0.78rem",
                              borderRadius: "8px",
                              padding: "0 14px",
                              display: "flex",
                              alignItems: "center",
                              gap: "6px",
                            }}
                            onClick={async () => {
                              setRecordStatus("recording");
                              try {
                                await startRecordingHotkey();
                              } catch (err) {
                                console.error(err);
                                setRecordStatus("idle");
                              }
                            }}
                          >
                            <span>{t.hotkeyRecordBtn}</span>
                          </button>
                        </>
                      )}
                    </div>
                  </div>

                  <label className="setting-row">
                    <span>{t.autoSubmit}</span>
                    <select
                      value={autoSubmit ? "true" : "false"}
                      onChange={(event) => {
                        const val = event.target.value === "true";
                        void handleHotkeySettingsChange(alwaysOn, hotkeyType, val);
                      }}
                      className="glass-input"
                    >
                      <option value="false">{t.autoSubmitDisabled}</option>
                      <option value="true">{t.autoSubmitEnabled}</option>
                    </select>
                  </label>
                </div>
              </section>
            </div>
          )}

          {activeTab === "general" && (
            <div className="flex flex-col gap-6">
              <section className="settings-section">
                <div className="settings-section-heading">
                  <Settings className="h-4 w-4" />
                  <span>{t.tabGeneral}</span>
                </div>

                <div className="settings-grid">
                  <label className="setting-row">
                    <span>{t.selectLanguage}</span>
                    <select
                      value={language}
                      onChange={(event) => {
                        const newLang = event.target.value as Language;
                        onLanguageChange(newLang);
                        localStorage.setItem("transcribe_language", newLang);
                      }}
                      className="glass-input"
                    >
                      <option value="auto">{t.langAuto}</option>
                      <option value="en">{t.langEn}</option>
                      <option value="ru">{t.langRu}</option>
                      <option value="uk">{t.langUk}</option>
                      <option value="zh">{t.langZh}</option>
                      <option value="es">{t.langEs}</option>
                    </select>
                  </label>

                  <label className="setting-row">
                    <span>{t.qualityProfile}</span>
                    <select
                      value={quality}
                      onChange={(event) => setQuality(event.target.value)}
                      className="glass-input"
                    >
                      <option value="fast">{t.qualityFast}</option>
                      <option value="balanced">{t.qualityBalanced}</option>
                      <option value="accurate">{t.qualityAccurate}</option>
                    </select>
                  </label>

                  <label className="setting-row">
                    <span>{t.computeBackend}</span>
                    <select
                      value={backend}
                      onChange={(event) =>
                        setBackend(event.target.value as ModelStatus["backend"])
                      }
                      className="glass-input"
                    >
                      <option value="auto">Auto</option>
                      <option value="cuda">CUDA</option>
                      <option value="cpu">CPU</option>
                      <option value="metal">Metal</option>
                    </select>
                  </label>
                </div>
              </section>
            </div>
          )}
        </main>
      </div>

      {error ? <div className="error-banner mt-5">{error}</div> : null}
    </div>
  );
}

function formatBytes(bytes: number): string {
  if (!bytes) return "0.0 MB";
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

function getJsKeyDisplayName(vkCode: number): string {
  switch (vkCode) {
    case 0x08:
      return "Backspace";
    case 0x09:
      return "Tab";
    case 0x0d:
      return "Enter";
    case 0x10:
    case 0xa0:
    case 0xa1:
      return "Shift";
    case 0x11:
    case 0xa2:
    case 0xa3:
      return "Control";
    case 0x12:
    case 0xa4:
    case 0xa5:
      return "Alt";
    case 0x13:
      return "Pause";
    case 0x14:
      return "Caps Lock";
    case 0x1b:
      return "Escape";
    case 0x20:
      return "Space";
    case 0x21:
      return "Page Up";
    case 0x22:
      return "Page Down";
    case 0x23:
      return "End";
    case 0x24:
      return "Home";
    case 0x25:
      return "Left Arrow";
    case 0x26:
      return "Up Arrow";
    case 0x27:
      return "Right Arrow";
    case 0x28:
      return "Down Arrow";
    case 0x2c:
      return "Print Screen";
    case 0x2d:
      return "Insert";
    case 0x2e:
      return "Delete";
    case 0x5f:
      return "Sleep";
    case 0x90:
      return "Num Lock";
    case 0x91:
      return "Scroll Lock";
    case 0xa6:
      return "Browser Back";
    case 0xa7:
      return "Browser Forward";
    case 0xa8:
      return "Browser Refresh";
    case 0xa9:
      return "Browser Stop";
    case 0xaa:
      return "Browser Search";
    case 0xab:
      return "Browser Favorites";
    case 0xac:
      return "Browser Home";
    case 0xad:
      return "Volume Mute";
    case 0xae:
      return "Volume Down";
    case 0xaf:
      return "Volume Up";
    case 0xb0:
      return "Next Track";
    case 0xb1:
      return "Previous Track";
    case 0xb2:
      return "Stop Media";
    case 0xb3:
      return "Play/Pause Media";
    case 0xba:
      return ";";
    case 0xbb:
      return "=";
    case 0xbc:
      return ",";
    case 0xbd:
      return "-";
    case 0xbe:
      return ".";
    case 0xbf:
      return "/";
    case 0xc0:
      return "`";
    case 0xdb:
      return "[";
    case 0xdc:
      return "\\";
    case 0xdd:
      return "]";
    case 0xde:
      return "'";
  }

  if (vkCode >= 0x30 && vkCode <= 0x39) {
    return String.fromCharCode(vkCode);
  }
  if (vkCode >= 0x41 && vkCode <= 0x5a) {
    return String.fromCharCode(vkCode);
  }
  if (vkCode >= 0x60 && vkCode <= 0x69) {
    return `Num ${vkCode - 0x60}`;
  }
  if (vkCode === 0x6a) return "Num *";
  if (vkCode === 0x6b) return "Num +";
  if (vkCode === 0x6c) return "Num Separator";
  if (vkCode === 0x6d) return "Num -";
  if (vkCode === 0x6e) return "Num .";
  if (vkCode === 0x6f) return "Num /";

  if (vkCode >= 0x70 && vkCode <= 0x87) {
    return `F${vkCode - 0x70 + 1}`;
  }

  return `Key 0x${vkCode.toString(16).toUpperCase()}`;
}
