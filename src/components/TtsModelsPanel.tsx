import { CheckCircle2, ChevronDown, Download, FolderOpen, Link2, Loader2, Plus, Trash2, Volume2, X } from "lucide-react";
import { useEffect, useState } from "react";
import {
  cancelDownload,
  deleteCustomTtsModel,
  deleteTtsVoice,
  downloadCustomTtsModel,
  downloadTtsVoice,
  errorMessage,
  getDownloadProgress,
  getTtsStatus,
  listCustomTtsModels,
  listTtsVoices,
  openTtsModelsFolder,
  setActiveCustomTtsModel,
  setCustomTtsModelVoice,
  setTtsVoice,
} from "../lib/tauri";
import type { TtsCustomModelInfo, TtsStatus, TtsVoiceInfo } from "../lib/types";

interface TtsModelsPanelProps {
  onError: (message: string) => void;
}

export function TtsModelsPanel({ onError }: TtsModelsPanelProps) {
  const [status, setStatus] = useState<TtsStatus | null>(null);
  const [voices, setVoices] = useState<TtsVoiceInfo[]>([]);
  const [customModels, setCustomModels] = useState<TtsCustomModelInfo[]>([]);
  const [loading, setLoading] = useState(true);
  const [activeDownload, setActiveDownload] = useState("");
  const [downloadProgress, setDownloadProgress] = useState<Record<string, { downloadedBytes: number; totalBytes: number; percentage: number }>>({});
  const [expandedModel, setExpandedModel] = useState(true);
  const [customFormOpen, setCustomFormOpen] = useState(false);
  const [customUrl, setCustomUrl] = useState("");
  const [customName, setCustomName] = useState("");
  const [customLicense, setCustomLicense] = useState("");
  const [customEngine, setCustomEngine] = useState("auto");

  useEffect(() => {
    void refresh();
  }, []);

  useEffect(() => {
    if (!activeDownload) return;
    const interval = window.setInterval(async () => {
      try {
        const progress = await getDownloadProgress(activeDownload);
        if (!progress) return;
        setDownloadProgress((prev) => ({
          ...prev,
          [activeDownload]: {
            downloadedBytes: progress.downloadedBytes,
            totalBytes: progress.totalBytes,
            percentage: Math.round(progress.percentage),
          },
        }));
      } catch (error) {
        console.error("Could not poll TTS download progress:", error);
      }
    }, 250);

    return () => window.clearInterval(interval);
  }, [activeDownload]);

  async function refresh() {
    try {
      setLoading(true);
      onError("");
      const [nextStatus, nextVoices, nextCustomModels] = await Promise.all([
        getTtsStatus(),
        listTtsVoices(),
        listCustomTtsModels(),
      ]);
      setStatus(nextStatus);
      setVoices(nextVoices);
      setCustomModels(nextCustomModels);
    } catch (error) {
      onError(errorMessage(error));
    } finally {
      setLoading(false);
    }
  }

  async function installVoice(voiceId: string) {
    try {
      onError("");
      setActiveDownload(voiceId);
      setStatus(await downloadTtsVoice(voiceId));
      await refresh();
    } catch (error) {
      const message = errorMessage(error);
      if (!message.toLowerCase().includes("cancel")) onError(message);
    } finally {
      setActiveDownload("");
      clearProgress(voiceId);
    }
  }

  async function chooseVoice(voiceId: string) {
    try {
      onError("");
      setStatus(await setTtsVoice(voiceId));
      setVoices(await listTtsVoices());
    } catch (error) {
      onError(errorMessage(error));
    }
  }

  async function removeVoice(voiceId: string) {
    try {
      onError("");
      setStatus(await deleteTtsVoice(voiceId));
      setVoices(await listTtsVoices());
    } catch (error) {
      onError(errorMessage(error));
    }
  }

  async function installCustomModel() {
    const url = customUrl.trim();
    if (!url) {
      onError("Вставьте ссылку на файл голосовой модели.");
      return;
    }
    try {
      onError("");
      setActiveDownload("custom-tts");
      await downloadCustomTtsModel({
        url,
        name: customName.trim() || undefined,
        license: customLicense.trim() || undefined,
        engine: customEngine,
      });
      setCustomUrl("");
      setCustomName("");
      setCustomLicense("");
      setCustomEngine("auto");
      setCustomFormOpen(false);
      setCustomModels(await listCustomTtsModels());
    } catch (error) {
      const message = errorMessage(error);
      if (!message.toLowerCase().includes("cancel")) onError(message);
    } finally {
      setActiveDownload("");
      clearProgress("custom-tts");
    }
  }

  async function removeCustomModel(modelId: string) {
    try {
      onError("");
      await deleteCustomTtsModel(modelId);
      setCustomModels(await listCustomTtsModels());
    } catch (error) {
      onError(errorMessage(error));
    }
  }

  async function chooseCustomModelActive(modelId: string) {
    try {
      onError("");
      setStatus(await setActiveCustomTtsModel(modelId));
      setVoices(await listTtsVoices());
      setCustomModels(await listCustomTtsModels());
    } catch (error) {
      onError(errorMessage(error));
    }
  }

  async function chooseCustomModelVoice(modelId: string, voiceId: string) {
    if (!voiceId) return;
    try {
      onError("");
      await setCustomTtsModelVoice(modelId, voiceId);
      setCustomModels(await listCustomTtsModels());
    } catch (error) {
      onError(errorMessage(error));
    }
  }

  async function openFolder() {
    try {
      await openTtsModelsFolder();
    } catch (error) {
      onError(errorMessage(error));
    }
  }

  async function cancelActiveDownload() {
    try {
      await cancelDownload();
    } catch (error) {
      onError(errorMessage(error));
    }
  }

  function clearProgress(id: string) {
    setDownloadProgress((prev) => {
      const next = { ...prev };
      delete next[id];
      return next;
    });
  }

  const engineDownloading = activeDownload === "piper-engine";
  const customDownloading = activeDownload === "custom-tts";
  const downloadedVoices = voices.filter((voice) => voice.isDownloaded);
  const activeVoiceId = status?.selectedVoiceId || "";
  // The built-in dropdown must only reflect a built-in selection; when a custom
  // model is active, leave it unselected.
  const builtinSelectedId = downloadedVoices.some((voice) => voice.id === activeVoiceId) ? activeVoiceId : "";

  return (
    <div className="flex flex-col gap-6">
      <section className="settings-section">
        <div className="settings-section-heading">
          <Volume2 className="h-4 w-4" />
          <span>TTS модели</span>
        </div>

        <div className="model-row-container">
          <div className="model-row">
            <div className="model-row-main">
              <div className="model-row-title">
                <span>Piper TTS</span>
                <span>выбрана</span>
                <span>MIT</span>
                {status?.engineExists ? <CheckCircle2 className="h-4 w-4 text-emerald-600 dark:text-emerald-400" /> : null}
              </div>
              <div className="model-row-description">
                Встроенная локальная TTS-модель. Голоса скачиваются отдельно и подключаются к этой модели.
              </div>
              <div className="model-row-file">
                {status?.engineExists ? `Встроенный движок: ${status.enginePath}` : "Встроенный Piper engine не найден"}
              </div>
            </div>
            <div className="flex gap-2">
              <button
                className="secondary-button"
                onClick={openFolder}
                style={smallButtonStyle}
                title="Открыть папку TTS"
              >
                <FolderOpen className="h-4 w-4" />
              </button>
              <button
                className="secondary-button"
                onClick={() => setExpandedModel((value) => !value)}
                style={{ ...smallButtonStyle, minWidth: "132px" }}
              >
                <ChevronDown className={`h-4 w-4 transition-transform ${expandedModel ? "rotate-180" : ""}`} />
                <span>{expandedModel ? "Свернуть" : "Голоса"}</span>
              </button>
            </div>
          </div>

          {expandedModel ? (
            <div className="tts-voices-panel">
              <label className="setting-row">
                <span>Активный голос</span>
                <select
                  className="glass-input"
                  value={builtinSelectedId}
                  onChange={(event) => {
                    const voiceId = event.target.value;
                    if (voiceId) void chooseVoice(voiceId);
                  }}
                  disabled={downloadedVoices.length === 0}
                >
                  <option value="">{downloadedVoices.length ? "Выберите голос" : "Сначала скачайте голос"}</option>
                  {downloadedVoices.map((voice) => (
                    <option key={voice.id} value={voice.id}>
                      {voice.name} · {voice.locale} · {voice.quality}
                    </option>
                  ))}
                </select>
              </label>

              <div className="settings-section-heading tts-voices-heading">
                <Download className="h-4 w-4" />
                <span>Голоса модели Piper</span>
              </div>

              <div className="model-list">
                {loading ? (
                  <div className="model-row model-row-muted">
                    <Loader2 className="h-4 w-4 animate-spin" />
                    Загрузка голосов...
                  </div>
                ) : (
                  voices.map((voice) => {
                    const downloading = activeDownload === voice.id;
                    const percentage = downloadProgress[voice.id]?.percentage || 0;
                    return (
                      <div className="model-row-container" key={voice.id}>
                        <div className="model-row">
                          <div className="model-row-main">
                            <div className="model-row-title">
                              <span>{voice.name}</span>
                              <span>{voice.locale}</span>
                              <span>{voice.quality}</span>
                              <span>{voice.license}</span>
                              {voice.isSelected ? <CheckCircle2 className="h-4 w-4 text-emerald-600 dark:text-emerald-400" /> : null}
                            </div>
                            <div className="model-row-description">{voiceDescription(voice)}</div>
                            <div className="model-row-file">{voice.id} · {voice.repository}</div>
                          </div>
                          {voice.isDownloaded ? (
                            <div className="flex gap-2">
                              <button
                                className="secondary-button"
                                onClick={() => chooseVoice(voice.id)}
                                disabled={voice.isSelected}
                                style={smallButtonStyle}
                              >
                                {voice.isSelected ? "Выбран" : "Выбрать"}
                              </button>
                              <button
                                className="secondary-button hover:bg-rose-50 hover:text-rose-600 dark:hover:bg-rose-950/20"
                                onClick={() => removeVoice(voice.id)}
                                disabled={voice.isSelected}
                                title="Удалить голос с диска"
                                style={smallButtonStyle}
                              >
                                <Trash2 className="h-4 w-4" />
                              </button>
                            </div>
                          ) : (
                            <button
                              className={downloading ? "secondary-button border-rose-300 text-rose-600" : "primary-button"}
                              onClick={() => (downloading ? cancelActiveDownload() : installVoice(voice.id))}
                              disabled={Boolean(activeDownload) && !downloading}
                              style={{ ...smallButtonStyle, minWidth: "125px" }}
                            >
                              {downloading ? <X className="h-4 w-4" /> : <Download className="h-4 w-4" />}
                              <span>{downloading ? "Отмена" : "Скачать"}</span>
                            </button>
                          )}
                        </div>
                        {downloading ? <ProgressLine progress={downloadProgress[voice.id]} percentage={percentage} /> : null}
                      </div>
                    );
                  })
                )}
              </div>
            </div>
          ) : null}
        </div>
      </section>

      <section className="settings-section">
        <div className="mb-4 flex items-center justify-between gap-3">
          <div className="settings-section-heading mb-0">
            <Link2 className="h-4 w-4" />
            <span>Другие голосовые модели</span>
          </div>
          <button
            className="secondary-button"
            onClick={() => setCustomFormOpen((value) => !value)}
            style={{ ...smallButtonStyle, minWidth: "158px" }}
          >
            <Plus className="h-4 w-4" />
            <span>Добавить модель</span>
          </button>
        </div>

        <div className="model-row-description mb-4">
          Вставьте прямую ссылку на файл модели. Приложение скачает и сохранит её в TTS-папку; запуск зависит от того, есть ли подходящий runtime для формата модели.
        </div>

        {customFormOpen ? (
          <div className="tts-voices-panel">
            <label className="setting-row">
              <span>Ссылка на файл</span>
              <input
                className="glass-input"
                value={customUrl}
                onChange={(event) => setCustomUrl(event.target.value)}
                placeholder="https://example.com/model.pt"
              />
            </label>
            <div className="settings-grid">
              <label className="setting-row">
                <span>Название</span>
                <input
                  className="glass-input"
                  value={customName}
                  onChange={(event) => setCustomName(event.target.value)}
                  placeholder="Определить из файла"
                />
              </label>
              <label className="setting-row">
                <span>Лицензия</span>
                <input
                  className="glass-input"
                  value={customLicense}
                  onChange={(event) => setCustomLicense(event.target.value)}
                  placeholder="Например MIT, CC-NC-BY или своя"
                />
              </label>
              <label className="setting-row">
                <span>Тип</span>
                <select
                  className="glass-input"
                  value={customEngine}
                  onChange={(event) => setCustomEngine(event.target.value)}
                >
                  <option value="auto">Определить автоматически</option>
                  <option value="Piper / ONNX">Piper / ONNX</option>
                  <option value="PyTorch / Silero-compatible">PyTorch / Silero-compatible</option>
                  <option value="TorchScript">TorchScript</option>
                  <option value="Custom TTS model">Другая TTS-модель</option>
                </select>
              </label>
            </div>
            <div className="library-editor-actions">
              <button
                className={customDownloading ? "secondary-button border-rose-300 text-rose-600" : "primary-button"}
                onClick={() => (customDownloading ? cancelActiveDownload() : installCustomModel())}
                disabled={Boolean(activeDownload) && !customDownloading}
              >
                {customDownloading ? <X className="h-4 w-4" /> : <Download className="h-4 w-4" />}
                <span>{customDownloading ? "Отменить" : "Скачать и добавить"}</span>
              </button>
            </div>
            {customDownloading ? (
              <ProgressLine progress={downloadProgress["custom-tts"]} percentage={downloadProgress["custom-tts"]?.percentage || 0} />
            ) : null}
          </div>
        ) : null}

        <div className="model-list mt-4">
          {customModels.length === 0 ? (
            <div className="model-row model-row-muted">
              Внешние голосовые модели пока не добавлены.
            </div>
          ) : (
            customModels.map((model) => (
              <div className="model-row-container" key={model.id}>
                <div className="model-row">
                  <div className="model-row-main">
                    <div className="model-row-title">
                      <span>{model.name}</span>
                      <span>{model.engine}</span>
                      <span>{model.license}</span>
                      {model.isSelected ? (
                        <CheckCircle2 className="h-4 w-4 text-emerald-600 dark:text-emerald-400" />
                      ) : null}
                    </div>
                    <div className="model-row-description">{model.notes}</div>
                    <div className="model-row-file">{model.localPath || model.url}</div>
                  </div>
                  <div className="flex gap-2">
                    <button
                      className="secondary-button"
                      onClick={() => chooseCustomModelActive(model.id)}
                      disabled={!model.synthesisSupported || model.isSelected}
                      title={
                        model.synthesisSupported
                          ? "Сделать активным голосом"
                          : "Модель пока нельзя синтезировать — см. описание ниже"
                      }
                      style={smallButtonStyle}
                    >
                      {model.isSelected ? "Выбрана" : "Выбрать"}
                    </button>
                    <button
                      className="secondary-button hover:bg-rose-50 hover:text-rose-600 dark:hover:bg-rose-950/20"
                      onClick={() => removeCustomModel(model.id)}
                      title="Удалить модель с диска"
                      style={smallButtonStyle}
                    >
                      <Trash2 className="h-4 w-4" />
                    </button>
                  </div>
                </div>
                {model.voices.length > 0 ? (
                  <div className="tts-custom-voice-panel">
                    <label className="setting-row">
                      <span>Голос / speaker</span>
                      <select
                        className="glass-input"
                        value={model.selectedVoiceId || ""}
                        onChange={(event) => chooseCustomModelVoice(model.id, event.target.value)}
                      >
                        {model.voices.map((voice) => (
                          <option key={voice.id} value={voice.id}>
                            {voice.name} · #{voice.id}
                          </option>
                        ))}
                      </select>
                    </label>
                  </div>
                ) : null}
              </div>
            ))
          )}
        </div>
      </section>
    </div>
  );
}

function ProgressLine({ progress, percentage }: { progress?: { downloadedBytes: number; totalBytes: number }; percentage: number }) {
  return (
    <div className="download-progress-container">
      <div className="mb-1 flex justify-between text-[0.72rem] font-semibold text-slate-500 dark:text-slate-300">
        <span>Загрузка...</span>
        <span>{progress ? `${formatBytes(progress.downloadedBytes)} / ${formatBytes(progress.totalBytes)} (${percentage}%)` : "0%"}</span>
      </div>
      <div className="download-progress-bar">
        <div className="download-progress-bar-fill" style={{ width: `${percentage}%` }} />
      </div>
    </div>
  );
}

function formatBytes(bytes = 0) {
  if (!bytes) return "0.0 MB";
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

function voiceDescription(voice: TtsVoiceInfo) {
  if (voice.id === "ru_RU-dmitri-medium") {
    return "Русский мужской голос Piper из rhasspy/piper-voices.";
  }
  if (voice.id === "ru_RU-denis-medium") {
    return "Русский мужской голос Piper с другим тембром из rhasspy/piper-voices.";
  }
  if (voice.id === "ru_RU-irina-medium") {
    return "Русский женский голос Piper из rhasspy/piper-voices.";
  }
  if (voice.id === "ru_RU-ruslan-medium") {
    return "Русский мужской голос Piper с более плотным тембром из rhasspy/piper-voices.";
  }
  return voice.description;
}

const smallButtonStyle = {
  height: "2.1rem",
  minHeight: "2.1rem",
  padding: "0 0.8rem",
  borderRadius: "8px",
  fontSize: "0.78rem",
} as const;
