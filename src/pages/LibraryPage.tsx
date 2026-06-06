import { Clipboard, Copy, History, Loader2, Play, RefreshCw, Trash2, Volume2 } from "lucide-react";
import { useEffect, useMemo, useRef, useState } from "react";
import {
  deleteHistoryItem,
  errorMessage,
  fileAssetUrl,
  getHistory,
  getTtsStatus,
  listTtsVoices,
  synthesizeSpeech,
  writeClipboard,
} from "../lib/tauri";
import type { HistoryItem, TtsStatus, TtsVoiceInfo } from "../lib/types";
import { formatDate, formatDuration, languageLabel } from "../lib/format";

type LibraryTab = "history" | "speech";

export function LibraryPage() {
  const [activeTab, setActiveTab] = useState<LibraryTab>("history");
  const [historyItems, setHistoryItems] = useState<HistoryItem[]>([]);
  const [ttsStatus, setTtsStatus] = useState<TtsStatus | null>(null);
  const [voices, setVoices] = useState<TtsVoiceInfo[]>([]);
  const [text, setText] = useState("");
  const [message, setMessage] = useState("");
  const [loadingHistory, setLoadingHistory] = useState(true);
  const [speaking, setSpeaking] = useState(false);
  const audioRef = useRef<HTMLAudioElement | null>(null);

  useEffect(() => {
    void loadHistory();
    void loadTts();
  }, []);

  async function loadHistory() {
    try {
      setLoadingHistory(true);
      setHistoryItems(await getHistory());
    } catch (error) {
      setMessage(errorMessage(error));
    } finally {
      setLoadingHistory(false);
    }
  }

  async function loadTts() {
    try {
      const [status, voiceList] = await Promise.all([getTtsStatus(), listTtsVoices()]);
      setTtsStatus(status);
      setVoices(voiceList);
    } catch (error) {
      setMessage(errorMessage(error));
    }
  }

  async function copyText(value: string) {
    try {
      await writeClipboard(value);
      setMessage("Скопировано в буфер.");
    } catch (error) {
      setMessage(errorMessage(error));
    }
  }

  async function removeHistoryItem(id: number) {
    try {
      await deleteHistoryItem(id);
      await loadHistory();
    } catch (error) {
      setMessage(errorMessage(error));
    }
  }

  function useForSpeech(value: string) {
    setText(value);
    setActiveTab("speech");
    setMessage("");
  }

  async function speak() {
    if (!text.trim()) return;
    try {
      setSpeaking(true);
      setMessage("");
      const result = await synthesizeSpeech(text);
      const audio = new Audio(fileAssetUrl(result.audioPath));
      audioRef.current?.pause();
      audioRef.current = audio;
      audio.onended = () => setSpeaking(false);
      audio.onerror = () => {
        setSpeaking(false);
        setMessage("Не удалось воспроизвести сгенерированный WAV.");
      };
      await audio.play();
      setMessage(`Озвучено голосом ${result.voiceName}.`);
    } catch (error) {
      setSpeaking(false);
      setMessage(errorMessage(error));
    }
  }

  const groupedHistory = useMemo(() => groupHistoryByDay(historyItems), [historyItems]);
  const selectedVoice = voices.find((voice) => voice.id === ttsStatus?.selectedVoiceId);
  const canSpeak = Boolean(text.trim() && ttsStatus?.engineExists && ttsStatus.selectedVoiceDownloaded);

  return (
    <div className="settings-panel-content flex flex-1 min-h-0 flex-col pt-2">
      <div className="settings-tabs-container library-tabs-container">
        <aside className="settings-sidebar">
          <button
            className={`sidebar-tab-btn ${activeTab === "history" ? "active" : ""}`}
            onClick={() => setActiveTab("history")}
          >
            <History className="h-4 w-4" />
            <span>История</span>
          </button>
          <button
            className={`sidebar-tab-btn ${activeTab === "speech" ? "active" : ""}`}
            onClick={() => setActiveTab("speech")}
          >
            <Volume2 className="h-4 w-4" />
            <span>Озвучивание</span>
          </button>
        </aside>

        <main className="settings-tab-content">
          {message ? <div className={isErrorMessage(message) ? "error-banner mb-4" : "success-banner mb-4"}>{message}</div> : null}

          {activeTab === "history" ? (
            <section className="settings-section">
              <div className="mb-4 flex items-center justify-between gap-3">
                <div className="settings-section-heading mb-0">
                  <History className="h-4 w-4" />
                  <span>История распознавания</span>
                </div>
                <button className="secondary-button library-small-button" onClick={loadHistory}>
                  <RefreshCw className="h-4 w-4" />
                  <span>Обновить</span>
                </button>
              </div>

              {loadingHistory ? (
                <div className="model-row model-row-muted">
                  <Loader2 className="h-4 w-4 animate-spin" />
                  Загрузка истории...
                </div>
              ) : groupedHistory.length === 0 ? (
                <div className="empty-state">Пока нет сохранённых распознаваний.</div>
              ) : (
                <div className="history-day-list">
                  {groupedHistory.map((group) => (
                    <div key={group.day} className="history-day-group">
                      <div className="history-day-title">{group.day}</div>
                      <div className="space-y-3">
                        {group.items.map((item) => (
                          <article key={item.id} className="history-item">
                            <div className="min-w-0 flex-1">
                              <div className="mb-2 flex flex-wrap items-center gap-2 text-xs text-slate-500 dark:text-slate-400">
                                <span>{formatTime(item.createdAt)}</span>
                                <span>{languageLabel(item.language)}</span>
                                <span>{formatDuration(item.durationSeconds)}</span>
                                <span>{item.modelName}</span>
                              </div>
                              <p className="line-clamp-4 whitespace-pre-wrap text-sm leading-6 text-slate-800 dark:text-slate-100">{item.text}</p>
                            </div>
                            <div className="flex shrink-0 gap-2">
                              <button className="icon-button" onClick={() => useForSpeech(item.text)} aria-label="Озвучить" title="Озвучить">
                                <Volume2 className="h-4 w-4" />
                              </button>
                              <button className="icon-button" onClick={() => copyText(item.text)} aria-label="Скопировать" title="Скопировать">
                                <Copy className="h-4 w-4" />
                              </button>
                              <button className="icon-button danger" onClick={() => removeHistoryItem(item.id)} aria-label="Удалить" title="Удалить">
                                <Trash2 className="h-4 w-4" />
                              </button>
                            </div>
                          </article>
                        ))}
                      </div>
                    </div>
                  ))}
                </div>
              )}
            </section>
          ) : (
            <section className="settings-section">
              <div className="settings-section-heading">
                <Volume2 className="h-4 w-4" />
                  <span>Озвучивание текста</span>
              </div>

              <div className="library-speech-grid">
                <div className="library-editor-panel">
                  <textarea
                    className="transcript-editor library-tts-editor"
                    value={text}
                    onChange={(event) => setText(event.target.value)}
                    placeholder="Вставьте текст, который нужно озвучить..."
                  />
                  <div className="library-editor-actions">
                    <button className="secondary-button" onClick={() => copyText(text)} disabled={!text.trim()}>
                      <Clipboard className="h-4 w-4" />
                      <span>Скопировать</span>
                    </button>
                    <button className="primary-button" onClick={speak} disabled={!canSpeak || speaking}>
                      {speaking ? <Loader2 className="h-4 w-4 animate-spin" /> : <Play className="h-4 w-4" />}
                      <span>{speaking ? "Озвучиваем..." : "Озвучить"}</span>
                    </button>
                  </div>
                </div>

                <div className="library-voice-panel">
                  <div className="model-row">
                    <div className="model-row-main">
                      <div className="model-row-title">
                        <span>{selectedVoice ? `${selectedVoice.locale} ${selectedVoice.name}` : "Голос не выбран"}</span>
                        {selectedVoice ? <span>{selectedVoice.license}</span> : null}
                      </div>
                      <div className="model-row-description">
                        {selectedVoice ? voiceDescription(selectedVoice) : "Выберите и скачайте голос модели Piper во вкладке TTS модели в настройках."}
                      </div>
                      <div className="model-row-file">
                        {ttsStatus?.engineExists ? "TTS-модель Piper встроена" : "Встроенный Piper engine не найден"}
                      </div>
                    </div>
                  </div>
                  {!ttsStatus?.engineExists || !ttsStatus.selectedVoiceDownloaded ? (
                    <div className="error-banner">
                      Для озвучивания нужна TTS-модель Piper и скачанный выбранный голос. Это настраивается в Settings {"->"} TTS модели.
                    </div>
                  ) : null}
                </div>
              </div>
            </section>
          )}
        </main>
      </div>
    </div>
  );
}

function groupHistoryByDay(items: HistoryItem[]) {
  const groups = new Map<string, HistoryItem[]>();
  for (const item of items) {
    const key = formatDay(item.createdAt);
    const current = groups.get(key) || [];
    current.push(item);
    groups.set(key, current);
  }
  return Array.from(groups.entries()).map(([day, groupItems]) => ({ day, items: groupItems }));
}

function formatDay(value: string) {
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return formatDate(value);
  return new Intl.DateTimeFormat("ru-RU", {
    day: "2-digit",
    month: "long",
    year: "numeric",
  }).format(date);
}

function formatTime(value: string) {
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return value;
  return new Intl.DateTimeFormat("ru-RU", {
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
  }).format(date);
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

function isErrorMessage(message: string) {
  const normalized = message.toLowerCase();
  return normalized.includes("ошиб")
    || normalized.includes("failed")
    || normalized.includes("missing")
    || normalized.includes("not available")
    || normalized.includes("runtime is not available");
}
