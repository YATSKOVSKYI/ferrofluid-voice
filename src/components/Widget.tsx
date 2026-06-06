import { LibraryBig, Mic, Settings, Square, X } from "lucide-react";
import { useEffect, useState, useRef } from "react";
import type { PointerEvent as ReactPointerEvent } from "react";
import { motion, AnimatePresence, useAnimationControls } from "motion/react";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow, PhysicalPosition } from "@tauri-apps/api/window";
import { SoundWave } from "./SoundWave";
import { FerrofluidOrb } from "./FerrofluidOrb";
import { StatusPill } from "./StatusPill";
import { useLocales, translations, getAppLanguage } from "../lib/locales";
import type { Language } from "../lib/types";
import {
  closeCurrentWindow,
  errorMessage,
  startRecording,
  stopRecording,
  transcribeAudio,
  writeClipboard,
  saveTranscript,
  getHotkeySettings,
  getRecordingState,
  injectText,
  logMessage,
  openLibraryWindow,
} from "../lib/tauri";
import type { AppStatus, ModelStatus, TranscriptResult } from "../lib/types";

interface WidgetProps {
  language: Language;
  modelStatus: ModelStatus | null;
  onOpenSettings: () => void;
}

export function Widget({ language, modelStatus, onOpenSettings }: WidgetProps) {
  const [status, setStatus] = useState<AppStatus>("ready");
  const [result, setResult] = useState<TranscriptResult | null>(null);
  const [message, setMessage] = useState("");
  const [frequencies, setFrequencies] = useState<Uint8Array | null>(null);
  const [volume, setVolume] = useState<number>(0);
  const [recordingSeconds, setRecordingSeconds] = useState(0);
  const [showConfirmClose, setShowConfirmClose] = useState(false);
  const shellControls = useAnimationControls();

  const t = useLocales(language);

  const hasModel = Boolean(modelStatus?.exists);
  const hasEngine = Boolean(modelStatus?.engineExists);

  const [alwaysOn, setAlwaysOn] = useState(true);
  const [autoSubmit, setAutoSubmit] = useState(false);

  // Refs to avoid stale closures in Tauri global listeners
  const statusRef = useRef(status);
  statusRef.current = status;

  const languageRef = useRef(language);
  languageRef.current = language;

  const autoSubmitRef = useRef(autoSubmit);
  autoSubmitRef.current = autoSubmit;

  const alwaysOnRef = useRef(alwaysOn);
  alwaysOnRef.current = alwaysOn;

  const hotkeyStartInFlightRef = useRef(false);
  const hotkeyStopPendingRef = useRef(false);
  const hotkeyStopAlreadyStoppedRef = useRef(false);
  const hotkeySessionActiveRef = useRef(false);
  const dragCleanupRef = useRef<(() => void) | null>(null);

  const hasModelRef = useRef(hasModel);
  hasModelRef.current = hasModel;

  const hasEngineRef = useRef(hasEngine);
  hasEngineRef.current = hasEngine;

  // Sync hotkey settings on mount and when changed
  useEffect(() => {
    async function loadSettings() {
      try {
        const settings = await getHotkeySettings();
        setAlwaysOn(settings.alwaysOn);
        setAutoSubmit(settings.autoSubmit);
      } catch (error) {
        console.error("Failed to load hotkey settings in Widget:", error);
      }
    }
    void loadSettings();
  }, []);

  useEffect(() => {
    if (typeof window === "undefined" || !window.__TAURI_INTERNALS__) return;

    let unlisten: (() => void) | undefined;
    void listen<any>("hotkey-settings-changed", (event) => {
      setAlwaysOn(event.payload.alwaysOn);
      setAutoSubmit(event.payload.autoSubmit);
    }).then((handler) => {
      unlisten = handler;
    });

    return () => unlisten?.();
  }, []);

  // Listen for low-level OS hotkey events
  useEffect(() => {
    if (typeof window === "undefined" || !window.__TAURI_INTERNALS__) return;

    void logMessage("[Widget] useEffect for hotkeys entered!");
    let active = true;
    let unlistenStart: (() => void) | undefined;
    let unlistenStop: (() => void) | undefined;
    let unlistenError: (() => void) | undefined;

    const setupListeners = async () => {
      try {
        const uStart = await listen<any>("hotkey-start-recording", async (event) => {
          if (!active) return;
          const alreadyStarted = Boolean(event.payload?.alreadyStarted);
          void logMessage(`[hotkey-start-recording] Event received. alreadyStarted=${alreadyStarted}, statusRef.current=${statusRef.current}, hotkeySessionActive=${hotkeySessionActiveRef.current}`);
          if (statusRef.current !== "ready" && statusRef.current !== "done" && statusRef.current !== "error" && hotkeySessionActiveRef.current) {
            void logMessage(`[hotkey-start-recording] Returning early because hotkey session is already active.`);
            return;
          }

          hotkeyStopPendingRef.current = false;
          hotkeyStopAlreadyStoppedRef.current = false;
          hotkeySessionActiveRef.current = false;
          hotkeyStartInFlightRef.current = true;

          try {
            const win = getCurrentWindow();
            if (!alwaysOnRef.current) {
              await win.show();
              await win.unminimize();
              await win.setFocus();
              shellControls.set({ opacity: 0, scale: 0.94, y: 8, filter: "blur(10px)" });
              void shellControls.start({
                opacity: 1,
                scale: 1,
                y: 0,
                filter: "blur(0px)",
                transition: { type: "spring", stiffness: 420, damping: 30, mass: 0.7 },
              });
            }

            if (!hasModelRef.current) {
              void logMessage(`[hotkey-start-recording] ERROR: No model found!`);
              if (event.payload?.alreadyStarted) {
                await stopRecording().catch(() => undefined);
              }
              setStatus("error");
              const currentT = translations[getAppLanguage(languageRef.current)];
              setMessage(currentT.msgChooseModel);
              return;
            }

            if (!hasEngineRef.current) {
              void logMessage(`[hotkey-start-recording] ERROR: No engine found!`);
              if (event.payload?.alreadyStarted) {
                await stopRecording().catch(() => undefined);
              }
              setStatus("error");
              const currentT = translations[getAppLanguage(languageRef.current)];
              setMessage(currentT.msgEngineMissing);
              return;
            }

            setResult(null);
            setMessage("");
            if (!alreadyStarted) {
              try {
                void logMessage(`[hotkey-start-recording] Calling startRecording()`);
                await startRecording();
              } finally {
                hotkeyStartInFlightRef.current = false;
              }
            }
            statusRef.current = "recording";
            setStatus("recording");
            void logMessage(`[hotkey-start-recording] Recording ${alreadyStarted ? "already started on backend" : "started"}; setting hotkeySessionActive=true`);
            hotkeySessionActiveRef.current = true;
            void logMessage(`[hotkey-start-recording] hotkeyStopPending=${hotkeyStopPendingRef.current}`);
            if (hotkeyStopPendingRef.current) {
              const alreadyStopped = hotkeyStopAlreadyStoppedRef.current;
              hotkeyStopPendingRef.current = false;
              hotkeyStopAlreadyStoppedRef.current = false;
              if (hotkeySessionActiveRef.current) {
                void logMessage(`[hotkey-start-recording] Stop was pending. Transitioning to stop.`);
                hotkeySessionActiveRef.current = false;
                await finishHotkeyRecording(alreadyStopped);
              }
            }
          } catch (err) {
            void logMessage(`[hotkey-start-recording] Error caught: ${errorMessage(err)}`);
            console.error("Error in hotkey start-recording handler:", err);
            hotkeySessionActiveRef.current = false;
            hotkeyStopPendingRef.current = false;
            hotkeyStopAlreadyStoppedRef.current = false;
            statusRef.current = "error";
            setStatus("error");
            setMessage(errorMessage(err));
          } finally {
            hotkeyStartInFlightRef.current = false;
          }
        });

        if (!active) {
          uStart();
          return;
        }
        unlistenStart = uStart;
        void logMessage("[Widget] Listen to hotkey-start-recording registered!");

        const uStop = await listen<any>("hotkey-stop-recording", async (event) => {
          if (!active) return;
          const alreadyStopped = Boolean(event.payload?.alreadyStopped);
          void logMessage(`[hotkey-stop-recording] Event received. alreadyStopped=${alreadyStopped}, hotkeyStartInFlight=${hotkeyStartInFlightRef.current}, hotkeySessionActive=${hotkeySessionActiveRef.current}, statusRef.current=${statusRef.current}`);
          if (hotkeyStartInFlightRef.current) {
            void logMessage(`[hotkey-stop-recording] Start in flight. Queueing stop.`);
            hotkeyStopPendingRef.current = true;
            hotkeyStopAlreadyStoppedRef.current = alreadyStopped;
            return;
          }
          if (!hotkeySessionActiveRef.current || statusRef.current !== "recording") {
            void logMessage(`[hotkey-stop-recording] Ignoring stop because hotkeySessionActive is false or status is not recording.`);
            return;
          }
          hotkeySessionActiveRef.current = false;
          void logMessage(`[hotkey-stop-recording] Calling finishHotkeyRecording(${alreadyStopped})`);
          await finishHotkeyRecording(alreadyStopped);
        });

        if (!active) {
          uStop();
          return;
        }
        unlistenStop = uStop;
        void logMessage("[Widget] Listen to hotkey-stop-recording registered!");

        const uError = await listen<any>("hotkey-recording-error", (event) => {
          if (!active) return;
          if (statusRef.current !== "recording") {
            void logMessage(`[hotkey-recording-error] Ignoring error since status is ${statusRef.current}: ${event.payload?.message}`);
            return;
          }
          setStatus("error");
          const currentT = translations[getAppLanguage(languageRef.current)];
          setMessage(event.payload?.message || currentT.msgRecordingFailed);
        });

        if (!active) {
          uError();
          return;
        }
        unlistenError = uError;
        void logMessage("[Widget] Listen to hotkey-recording-error registered!");
      } catch (err) {
        void logMessage("[Widget] Listen setup failed: " + errorMessage(err));
      }
    };

    void setupListeners();

    return () => {
      active = false;
      unlistenStart?.();
      unlistenStop?.();
      unlistenError?.();
    };
  }, []);

  useEffect(() => {
    if (typeof window === "undefined" || !window.__TAURI_INTERNALS__) return;

    async function syncBackendRecordingState() {
      try {
        const isRecording = await getRecordingState();
        if (
          isRecording &&
          statusRef.current !== "recording" &&
          statusRef.current !== "processing"
        ) {
          setResult(null);
          setMessage("");
          statusRef.current = "recording";
          setStatus("recording");
        }
      } catch (error) {
        console.warn("Could not sync recording state:", error);
      }
    }

    window.addEventListener("focus", syncBackendRecordingState);
    document.addEventListener("visibilitychange", syncBackendRecordingState);
    void syncBackendRecordingState();

    return () => {
      window.removeEventListener("focus", syncBackendRecordingState);
      document.removeEventListener("visibilitychange", syncBackendRecordingState);
    };
  }, []);

  useEffect(() => {
    if (status !== "recording") {
      setFrequencies(null);
      setVolume(0);
      return;
    }

    let audioContext: AudioContext | null = null;
    let mediaStream: MediaStream | null = null;
    let animationFrameId = 0;

    async function startAudioAnalysis() {
      try {
        mediaStream = await navigator.mediaDevices.getUserMedia({ audio: true });
        
        const AudioContextClass = window.AudioContext || (window as any).webkitAudioContext;
        audioContext = new AudioContextClass();
        const analyser = audioContext.createAnalyser();
        analyser.fftSize = 64; // Gives 32 frequency bins
        
        const source = audioContext.createMediaStreamSource(mediaStream);
        source.connect(analyser);

        const dataArray = new Uint8Array(analyser.frequencyBinCount);

        const updateWave = () => {
          analyser.getByteFrequencyData(dataArray);
          
          // Capture raw frequency data
          setFrequencies(new Uint8Array(dataArray));
          
          // Calculate RMS volume level
          let sum = 0;
          for (let i = 0; i < dataArray.length; i++) {
            sum += dataArray[i] * dataArray[i];
          }
          const rms = Math.sqrt(sum / dataArray.length);
          // Map to 0 - 1.0 normalized value
          const normVol = Math.min(rms / 120, 1.0);
          setVolume(normVol);
          
          animationFrameId = requestAnimationFrame(updateWave);
        };

        updateWave();
      } catch (err) {
        console.warn("Could not access microphone for visualizer:", err);
      }
    }

    void startAudioAnalysis();

    return () => {
      if (animationFrameId) {
        cancelAnimationFrame(animationFrameId);
      }
      if (audioContext) {
        void audioContext.close();
      }
      if (mediaStream) {
        mediaStream.getTracks().forEach((track) => track.stop());
      }
    };
  }, [status]);

  useEffect(() => {
    function onKeyDown(event: KeyboardEvent) {
      const commandKey = event.metaKey || event.ctrlKey;
      if (commandKey && event.shiftKey && event.code === "Space") {
        event.preventDefault();
        void toggleRecording();
      }
    }

    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  });

  useEffect(() => {
    return () => {
      dragCleanupRef.current?.();
      dragCleanupRef.current = null;
    };
  }, []);

  useEffect(() => {
    if (status !== "recording") {
      setRecordingSeconds(0);
      return;
    }

    const interval = window.setInterval(() => {
      setRecordingSeconds((prev) => prev + 1);
    }, 1000);

    return () => window.clearInterval(interval);
  }, [status]);

  async function finishHotkeyRecording(alreadyStopped = false) {
    void logMessage(`[finishHotkeyRecording] Called. alreadyStopped=${alreadyStopped}, statusRef.current=${statusRef.current}`);
    if (statusRef.current !== "recording") {
      void logMessage(`[finishHotkeyRecording] Exiting because statusRef.current is not recording.`);
      return;
    }

    try {
      statusRef.current = "processing";
      setStatus("processing");
      if (!alreadyStopped) {
        try {
          void logMessage(`[finishHotkeyRecording] Calling stopRecording()`);
          await stopRecording();
        } catch (error) {
          const message = errorMessage(error);
          void logMessage(`[finishHotkeyRecording] stopRecording error: ${message}`);
          if (message.includes("Recording is not running")) {
            statusRef.current = "ready";
            setStatus("ready");
            const currentT = translations[getAppLanguage(languageRef.current)];
            setMessage(currentT.msgRecordDidNotStart);
            return;
          }
          throw error;
        }
      }

      const transcript = await transcribeAudio(languageRef.current);
      setResult(transcript);
      await saveTranscriptSafe(transcript);
      statusRef.current = "done";
      setStatus("done");

      if (transcript.text.trim()) {
        try {
          await writeClipboard(transcript.text);
          const currentT = translations[getAppLanguage(languageRef.current)];
          setMessage(currentT.msgCopied);
        } catch (e) {
          const currentT = translations[getAppLanguage(languageRef.current)];
          setMessage(`${currentT.msgTranscriptReady} ${errorMessage(e)}`);
        }
        await injectText(transcript.text, autoSubmitRef.current);
      } else {
        const currentT = translations[getAppLanguage(languageRef.current)];
        setMessage(currentT.msgTranscriptReady);
      }

      if (!alwaysOnRef.current) {
        // Let the user see the "Copied" status for a brief moment before hiding
        await new Promise((resolve) => setTimeout(resolve, 1000));
        const win = getCurrentWindow();
        await win.hide();
      }
    } catch (err) {
      void logMessage(`[finishHotkeyRecording] Error caught: ${errorMessage(err)}`);
      console.error("Error in hotkey stop-recording handler:", err);
      statusRef.current = "error";
      setStatus("error");
      setMessage(errorMessage(err));
    }
  }

  async function toggleRecording() {
    setMessage("");
    if (!hasModel) {
      setMessage(t.msgChooseModel);
      onOpenSettings();
      return;
    }
    if (!hasEngine) {
      setMessage(t.msgEngineMissing);
      return;
    }

    // Deactivate hotkey session immediately so that any hotkey release events don't trigger stop actions concurrently
    hotkeySessionActiveRef.current = false;

    try {
      if (status === "recording" || statusRef.current === "recording") {
        statusRef.current = "processing";
        setStatus("processing");
        await stopRecording();
        const transcript = await transcribeAudio(language);
        setResult(transcript);
        await saveTranscriptSafe(transcript);
        statusRef.current = "done";
        setStatus("done");
        if (transcript.text.trim()) {
          try {
            await writeClipboard(transcript.text);
            setMessage(t.msgCopied);
          } catch (error) {
            setMessage(`${t.msgTranscriptReady} ${errorMessage(error)}`);
          }
        } else {
          setMessage(t.msgTranscriptReady);
        }
      } else {
        setResult(null);
        await startRecording();
        statusRef.current = "recording";
        setStatus("recording");
      }
    } catch (error) {
      statusRef.current = "error";
      setStatus("error");
      setMessage(errorMessage(error));
    }
  }

  async function openLibrary() {
    try {
      await openLibraryWindow();
    } catch (error) {
      setMessage(errorMessage(error));
    }
  }

  async function saveTranscriptSafe(transcript: TranscriptResult) {
    if (!transcript.text.trim()) return;
    try {
      await saveTranscript(transcript);
    } catch (error) {
      console.warn("Could not save transcript to history:", error);
    }
  }

  function handleWindowDrag(event: ReactPointerEvent<HTMLElement>) {
    if (event.button !== 0 || !window.__TAURI_INTERNALS__) return;
    if (isInteractiveDragTarget(event.target)) return;

    const pointerId = event.pointerId;
    const startScreenX = event.screenX;
    const startScreenY = event.screenY;
    const dragSurface = event.currentTarget;
    event.preventDefault();

    try {
      dragSurface.setPointerCapture(pointerId);
    } catch {
      // Pointer capture can fail if the platform already handed the drag to the OS.
    }

    startManualWindowDrag(pointerId, startScreenX, startScreenY, dragSurface);
    void getCurrentWindow().startDragging().catch((error) => {
      console.warn("Could not start native window drag; manual dragging remains active.", error);
    });
  }

  function startManualWindowDrag(
    pointerId: number,
    startScreenX: number,
    startScreenY: number,
    dragSurface: HTMLElement,
  ) {
    const win = getCurrentWindow();
    let active = true;
    let origin: { x: number; y: number } | null = null;
    let latestPointer: PointerEvent | null = null;
    let animationFrame = 0;

    const cleanup = () => {
      active = false;
      window.removeEventListener("pointermove", onPointerMove, true);
      window.removeEventListener("pointerup", onPointerEnd, true);
      window.removeEventListener("pointercancel", onPointerEnd, true);
      window.removeEventListener("blur", cleanup);
      if (animationFrame) {
        window.cancelAnimationFrame(animationFrame);
      }
      try {
        if (dragSurface.hasPointerCapture(pointerId)) {
          dragSurface.releasePointerCapture(pointerId);
        }
      } catch {
        // Ignore capture release errors from platform-level drag completion.
      }
      if (dragCleanupRef.current === cleanup) {
        dragCleanupRef.current = null;
      }
    };

    const scheduleMove = (event: PointerEvent) => {
      latestPointer = event;
      if (!origin || animationFrame) return;

      animationFrame = window.requestAnimationFrame(() => {
        animationFrame = 0;
        if (!active || !origin || !latestPointer) return;

        const nextX = origin.x + latestPointer.screenX - startScreenX;
        const nextY = origin.y + latestPointer.screenY - startScreenY;
        void win.setPosition(new PhysicalPosition(Math.round(nextX), Math.round(nextY))).catch((error) => {
          console.warn("Could not manually move Ferrofluid Voice window.", error);
          cleanup();
        });
      });
    };

    const onPointerMove = (event: PointerEvent) => {
      if (event.pointerId !== pointerId) return;
      event.preventDefault();
      scheduleMove(event);
    };

    const onPointerEnd = (event: PointerEvent) => {
      if (event.pointerId === pointerId) cleanup();
    };

    dragCleanupRef.current?.();
    dragCleanupRef.current = cleanup;
    window.addEventListener("pointermove", onPointerMove, true);
    window.addEventListener("pointerup", onPointerEnd, true);
    window.addEventListener("pointercancel", onPointerEnd, true);
    window.addEventListener("blur", cleanup);

    void win.outerPosition().then((position) => {
      if (!active) return;
      origin = { x: position.x, y: position.y };
      if (latestPointer) scheduleMove(latestPointer);
    }).catch((error) => {
      console.warn("Could not read Ferrofluid Voice window position for manual drag.", error);
      cleanup();
    });
  }

  async function handleClose() {
    try {
      await closeCurrentWindow();
    } catch (error) {
      setMessage(errorMessage(error));
    }
  }

  return (
    <motion.section
      className="widget-shell"
      data-tauri-drag-region
      onPointerDownCapture={handleWindowDrag}
      initial={{ opacity: 1, scale: 1, y: 0, filter: "blur(0px)" }}
      animate={shellControls}
    >
      <div className="widget-container" data-tauri-drag-region>
        <div className="widget-left" data-tauri-drag-region>
          <button
            className={`widget-record ${status === "recording" ? "widget-record-active" : ""} ${status === "processing" ? "widget-record-processing" : ""}`}
            onClick={toggleRecording}
            disabled={status === "processing"}
            aria-label={status === "recording" ? "Stop recording" : "Start recording"}
            title={status === "recording" ? "Stop recording" : "Start recording"}
            style={status === "recording" ? ({ "--volume": volume } as React.CSSProperties) : undefined}
          >
            {status === "recording" ? (
              <FerrofluidOrb volume={volume} frequencies={frequencies} />
            ) : (
              <Mic className="h-4.5 w-4.5" />
            )}
          </button>
        </div>

        <div className="widget-middle" data-tauri-drag-region>
          {status === "recording" || status === "processing" ? (
            <>
              <SoundWave status={status} audioData={frequencies} />
              {status === "recording" && (
                <div style={{ display: "flex", alignItems: "center", justifyContent: "center" }} data-no-drag>
                  {formatTime(recordingSeconds).split("").map((char, idx) => (
                    <div
                      key={idx}
                      style={{
                        position: "relative",
                        height: "11px",
                        overflow: "hidden",
                        display: "flex",
                        alignItems: "center",
                        justifyContent: "center",
                        width: char === ":" ? "4px" : "7px", // pixel-stable width for perfect columnar alignment
                      }}
                    >
                      <AnimatePresence mode="popLayout">
                        <motion.span
                          key={char}
                          initial={{ opacity: 0, y: 6 }}
                          animate={{ opacity: 0.78, y: 0 }}
                          exit={{ opacity: 0, y: -6 }}
                          transition={{ duration: 0.22, ease: "easeInOut" }}
                          className="widget-timer"
                          style={{ display: "block", position: "absolute" }}
                        >
                          {char}
                        </motion.span>
                      </AnimatePresence>
                    </div>
                  ))}
                </div>
              )}
            </>
          ) : (
            <div className="widget-message" data-tauri-drag-region>
              {message || statusHint(status, hasModel, hasEngine, t)}
            </div>
          )}
        </div>

        <div className="widget-actions" data-no-drag>
          <button
            className="widget-action-btn widget-copy widget-copy-ready"
            onClick={openLibrary}
            aria-label="History and text to speech"
            title="History and text to speech"
          >
            <LibraryBig className="h-4 w-4" />
          </button>
          <button
            className="widget-action-btn"
            onClick={onOpenSettings}
            aria-label={t.tooltipSettings}
            title={t.tooltipSettings}
          >
            <Settings className="h-4 w-4" />
          </button>
          <button
            className="widget-action-btn widget-close"
            onClick={() => setShowConfirmClose(true)}
            aria-label={t.tooltipClose}
            title={t.tooltipClose}
          >
            <X className="h-4 w-4" />
          </button>
        </div>
      </div>

      <AnimatePresence>
        {showConfirmClose && (
          <motion.div
            initial={{ opacity: 0, scale: 0.94, y: 8 }}
            animate={{ opacity: 1, scale: 1, y: 0 }}
            exit={{ opacity: 0, scale: 0.96, y: 6 }}
            transition={{ type: "spring", stiffness: 380, damping: 26 }}
            style={{
              position: "absolute",
              inset: "4px",
              zIndex: 10,
              display: "flex",
              flexDirection: "row",
              alignItems: "center",
              justifyContent: "space-between",
              padding: "0 10px",
              borderRadius: "10px",
              border: "1px solid var(--glass-border)",
              background: "var(--glass-bg)",
              boxShadow: "inset 0 1.5px 0px var(--glass-highlight), 0 4px 15px rgba(0, 0, 0, 0.15)",
              backdropFilter: "blur(25px) saturate(1.3)",
              color: "rgb(15 23 42)",
            }}
            data-no-drag
          >
            <div
              style={{
                fontSize: "0.78rem",
                fontWeight: 700,
                opacity: 0.92,
              }}
              className="text-slate-950 dark:text-slate-50"
            >
              {t.exitTitle}
            </div>
            <div style={{ display: "flex", alignItems: "center", gap: "5px" }}>
              <button
                className="secondary-button"
                style={{
                  minHeight: "1.95rem",
                  height: "1.95rem",
                  padding: "0 0.65rem",
                  borderRadius: "8px",
                  fontSize: "0.74rem",
                }}
                onClick={() => setShowConfirmClose(false)}
              >
                {t.exitCancel}
              </button>
              <button
                className="primary-button"
                style={{
                  minHeight: "1.95rem",
                  height: "1.95rem",
                  padding: "0 0.65rem",
                  borderRadius: "8px",
                  fontSize: "0.74rem",
                  background: "#f43f5e",
                  borderColor: "#f43f5e",
                  color: "white",
                }}
                onClick={handleClose}
              >
                {t.exitSubmit}
              </button>
            </div>
          </motion.div>
        )}
      </AnimatePresence>
    </motion.section>
  );
}

function statusHint(status: AppStatus, hasModel: boolean, hasEngine: boolean, t: any) {
  if (!hasModel) return t.hintSelectModel;
  if (!hasEngine) return t.hintEngineMissing;
  if (status === "done") return t.hintCopied;
  if (status === "error") return t.hintCheckSettings;
  return t.hintReady;
}

function isInteractiveDragTarget(target: EventTarget | null) {
  if (!(target instanceof Element)) return false;
  return Boolean(target.closest("button,input,select,textarea,a,[role='button'],[data-no-drag]"));
}

function formatTime(seconds: number) {
  const mins = Math.floor(seconds / 60);
  const secs = seconds % 60;
  return `${mins}:${secs.toString().padStart(2, "0")}`;
}
