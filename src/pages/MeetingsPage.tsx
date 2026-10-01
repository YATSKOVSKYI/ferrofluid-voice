import { useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-dialog";
import { AudioLines, Check, ChevronDown, Download, FileAudio, FolderOpen, Headphones, Loader2, Pause, Play, Plus, Scissors, Search, ShieldCheck, Users, X } from "lucide-react";
import { analyzeMeeting, calibrateMeeting, cancelMeetingJob, exportMeetingFile, importMeeting, listMeetings, refineMeeting, meetingExport, meetingToolsStatus, saveMeeting, setupMeetingTools, timecode, transcribeMeeting } from "../lib/meetings";
import type { Meeting, MeetingProgress, MeetingSegment, MeetingToolsStatus } from "../lib/meetings";
import { errorMessage, fileAssetUrl, getModelStatus, openSettingsWindow } from "../lib/tauri";
import "../styles/meetings.css";
import { VoiceCalibrationPanel } from "./VoiceCalibrationPanel";

const colors = ["#007aff", "#af52de", "#ff9500", "#30b780", "#ff375f", "#32aaba"];

export function MeetingsPage() {
  const [meetings, setMeetings] = useState<Meeting[]>([]);
  const [meeting, setMeeting] = useState<Meeting | null>(null);
  const [tools, setTools] = useState<MeetingToolsStatus | null>(null);
  const [busy, setBusy] = useState(false);
  const [progress, setProgress] = useState<MeetingProgress | null>(null);
  const [error, setError] = useState("");
  const [notice, setNotice] = useState("");
  const [dirty, setDirty] = useState(false);
  const [speakerCount, setSpeakerCount] = useState(0);
  const [language, setLanguage] = useState("auto");
  const [search, setSearch] = useState("");
  const [filter, setFilter] = useState("all");
  const [onlyReview, setOnlyReview] = useState(false);
  const [position, setPosition] = useState(0);
  const [playing, setPlaying] = useState(false);
  const [exportFormat, setExportFormat] = useState<"txt" | "srt" | "json">("txt");
  const audio = useRef<HTMLAudioElement>(null);
  const stopAt = useRef<number | null>(null);
  const initialized = useRef(false);
  const pending = useRef(false);
  const textAreas = useRef(new Map<string, HTMLTextAreaElement>());

  useEffect(() => {
    if (!window.__TAURI_INTERNALS__) return;
    let disposed = false;
    const unlisten = listen<MeetingProgress>("meeting-progress", event => { if (!disposed) setProgress(event.payload); });
    const refresh = async () => {
      try {
        const status = await meetingToolsStatus();
        if (disposed) return;
        setTools(status);
        if (!pending.current) setBusy(status.busy);
        if (status.busy && status.progress) setProgress(status.progress);
        if (!initialized.current || (initialized.current && !status.busy && toolsBusy)) {
          const items = await listMeetings();
          if (disposed) return;
          setMeetings(items);
          if (!initialized.current) { setMeeting(items[0] || null); initialized.current = true; }
          else if (!pending.current) setMeeting(current => items.find(m => m.id === current?.id) || items[0] || null);
        }
        toolsBusy = status.busy;
      } catch (e) { if (!disposed) setError(errorMessage(e)); }
    };
    let toolsBusy = false;
    void refresh();
    const interval = window.setInterval(refresh, 2500);
    return () => { disposed = true; window.clearInterval(interval); void unlisten.then(fn => fn()); };
  }, []);

  useEffect(() => {
    const warn = (event: BeforeUnloadEvent) => {
      if (dirty) { event.preventDefault(); event.returnValue = ""; }
    };
    window.addEventListener("beforeunload", warn);
    return () => window.removeEventListener("beforeunload", warn);
  }, [dirty]);

  useEffect(() => { setPosition(0); setPlaying(false); stopAt.current = null; setSearch(""); setFilter("all"); setOnlyReview(false); }, [meeting?.id]);

  async function task(action: () => Promise<Meeting | void>, success?: string) {
    if (pending.current || busy) return;
    pending.current = true; setBusy(true); setError(""); setNotice(""); setProgress(null); audio.current?.pause();
    try {
      const result = await action();
      if (result) { setMeeting(result); setDirty(false); }
      setMeetings(await listMeetings());
      setTools(await meetingToolsStatus());
      if (success) setNotice(success);
    } catch (e) { setError(errorMessage(e)); }
    finally { pending.current = false; setBusy(false); setProgress(null); }
  }

  async function persist() {
    if (!meeting || !dirty) return;
    await saveMeeting(meeting); setDirty(false);
    setMeetings(items => items.map(m => m.id === meeting.id ? meeting : m));
  }

  async function pickFile() {
    if (!window.__TAURI_INTERNALS__) { setError("Импорт доступен в настольном Ferrofluid Voice."); return; }
    try {
      const path = await open({ multiple: false, title: "Запись конференции", filters: [{ name: "Аудио Zoom", extensions: ["m4a", "wav", "mp3", "flac", "ogg", "aac", "mp4"] }] });
      if (typeof path === "string") await task(async () => { await persist(); return importMeeting(path); });
    } catch (e) { setError(errorMessage(e)); }
  }

  async function choose(id: string) {
    if (busy) return;
    try { await persist(); audio.current?.pause(); setMeeting(meetings.find(m => m.id === id) || null); setDirty(false); setNotice(""); setError(""); }
    catch (e) { setError(errorMessage(e)); }
  }

  function update(next: Meeting) { setMeeting(next); setDirty(true); setNotice(""); }
  function updateSegment(id: string, patch: Partial<MeetingSegment>) {
    if (meeting) update({ ...meeting, segments: meeting.segments.map(s => s.id === id ? { ...s, ...patch } : s) });
  }
  function splitSegment(segment: MeetingSegment) {
    if (!meeting || position <= segment.start || position >= segment.end) return;
    const caret = textAreas.current.get(segment.id)?.selectionStart ?? Math.round(segment.text.length * (position - segment.start) / (segment.end - segment.start));
    const boundaries = Array.from(segment.text.matchAll(/\s+/g), match => match.index ?? 0).filter(index => index > 0 && index < segment.text.length - 1);
    const split = boundaries.sort((a, b) => Math.abs(a - caret) - Math.abs(b - caret))[0] ?? -1;
    if (split <= 0 || split >= segment.text.length - 1) { setError("Поставьте текстовый курсор между словами и выберите момент разделения на аудиодорожке."); return; }
    const first = { ...segment, end: position, text: segment.text.slice(0, split).trim(), needsReview: true };
    const second = { ...segment, id: `${segment.id}-${Date.now()}`, start: position, text: segment.text.slice(split).trim(), needsReview: true };
    update({ ...meeting, segments: meeting.segments.flatMap(s => s.id === segment.id ? [first, second] : [s]) });
  }
  function addSpeaker() {
    if (!meeting) return;
    const start = Math.min(position, Math.max(0, meeting.durationSeconds - 1));
    update({ ...meeting, speakers: [...meeting.speakers, { id: `speaker-manual-${Date.now()}`, name: `Участник ${meeting.speakers.length + 1}`, sampleStart: start, sampleEnd: Math.min(start + 8, meeting.durationSeconds) }] });
  }
  function mergeSpeaker(source: string, target: string) {
    if (!meeting || !target) return;
    update({ ...meeting, speakers: meeting.speakers.filter(s => s.id !== source),
      turns: meeting.turns.map(t => t.speakerId === source ? { ...t, speakerId: target } : t),
      segments: meeting.segments.map(s => s.speakerId === source ? { ...s, speakerId: target } : s) });
  }
  function color(id: string | null) { const index = meeting?.speakers.findIndex(s => s.id === id) ?? -1; return index >= 0 ? colors[index % colors.length] : "#8e8e93"; }
  async function play(start?: number, end?: number) {
    if (!audio.current) return;
    try {
      if (start !== undefined) audio.current.currentTime = start;
      stopAt.current = end ?? null;
      await audio.current.play();
    } catch { setError("Не удалось воспроизвести запись. Проверьте доступ к сохранённому аудио."); }
  }
  async function exportTranscript() {
    if (!meeting) return;
    try { await persist(); if (await exportMeetingFile(meetingExport(meeting, exportFormat), exportFormat)) setNotice("Транскрипт экспортирован."); }
    catch (e) { setError(errorMessage(e)); }
  }

  const segments = meeting?.segments.filter(s => (filter === "all" || s.speakerId === filter || (filter === "unknown" && !s.speakerId)) && (!onlyReview || s.needsReview) && s.text.toLocaleLowerCase().includes(search.toLocaleLowerCase())) || [];
  const reviewCount = meeting?.segments.filter(s => s.needsReview).length || 0;
  const step = !meeting ? 1 : !meeting.speakers.length ? 2 : !meeting.segments.length ? 3 : 4;

  return <div className="meeting-studio">
    <header className="meeting-heading">
      <div><div className="meeting-eyebrow"><AudioLines size={14} /> FERROFLUID STUDIO</div><h1>Каждый голос. Каждое слово.</h1><p>Превратите запись встречи в разговор с именами.</p></div>
      <button className="meeting-button primary" onClick={pickFile} disabled={busy}><Plus size={17} /> Новая запись</button>
    </header>
    <div className="meeting-steps" aria-label="Этапы обработки">{["Загрузить запись", "Различить голоса", "Назвать участников", "Транскрипт"].map((label, i) => <div key={label} className={step === i + 1 ? "current" : step > i + 1 ? "complete" : ""}><span>{step > i + 1 ? <Check size={12} /> : i + 1}</span>{label}</div>)}</div>
    {error && <div className="meeting-alert error" role="alert">{error}<button aria-label="Скрыть ошибку" onClick={() => setError("")}><X size={16} /></button></div>}
    {notice && <div className="meeting-alert" role="status"><Check size={16} />{notice}</div>}
    {busy && <div className="meeting-job" role="status"><div><Loader2 size={17} className="animate-spin" /><span>{progress?.message || "Подготовка…"}</span><button disabled={!progress} onClick={() => void cancelMeetingJob().catch(e => setError(errorMessage(e)))}>Отменить</button></div><progress max={100} value={progress?.percentage || 0} /></div>}
    {tools && (!tools.diarizationReady || !tools.pythonReady || !tools.ffmpegReady) && <div className="meeting-setup"><div className="meeting-setup-icon"><Download size={20} /></div><div><strong>Подготовьте студию один раз</strong><p>{!tools.pythonReady ? "Установите Python 3.11 для обработки записей. " : ""}{!tools.ffmpegReady ? "Для M4A нужен FFmpeg. " : ""}Движок голосов скачает модели и зависимости. Затем обработка работает без интернета.</p></div><button className="meeting-button" disabled={busy || !tools.pythonReady} onClick={() => void task(setupMeetingTools, "Локальный движок установлен.")}>{tools.diarizationReady ? "Переустановить" : "Установить движок"}</button></div>}

    {!meeting ? <section className="meeting-empty">
      <div className="meeting-empty-icon"><FileAudio size={34} strokeWidth={1.4} /></div><h2>Ваша встреча начинается здесь</h2><p>Выберите аудиозапись Zoom. Мы найдём голоса,<br />а вы дадите им имена.</p><button className="meeting-button primary" onClick={pickFile} disabled={busy}><FolderOpen size={17} /> Выбрать аудиофайл</button><div className="meeting-formats">M4A · WAV · MP3 · FLAC · OGG · AAC · MP4</div><div className="meeting-private"><ShieldCheck size={14} /> Аудио остаётся на вашем компьютере</div>
    </section> : <>
      <section className="meeting-recording">
        <div className="meeting-recording-icon"><FileAudio size={23} /></div><div className="meeting-recording-main"><input className="meeting-title-input" aria-label="Название конференции" value={meeting.title} disabled={busy} onChange={e => update({ ...meeting, title: e.target.value })} /><p>{meeting.sourceName} <span>·</span> {timecode(meeting.durationSeconds)} <span>·</span> {meeting.speakers.length || "—"} голоса</p></div>
        <div className="meeting-select-wrap"><select aria-label="Сохранённые конференции" disabled={busy} value={meeting.id} onChange={e => void choose(e.target.value)}>{meetings.map(m => <option value={m.id} key={m.id}>{m.title}</option>)}</select><ChevronDown size={14} /></div>
        <button className="meeting-button" disabled={busy || !dirty} onClick={() => void task(async () => { await persist(); }, "Изменения сохранены.")}><Check size={15} /> {dirty ? "Сохранить" : "Сохранено"}</button>
      </section>
      <section className="meeting-player">
        <audio ref={audio} key={meeting.id} src={fileAssetUrl(meeting.audioPath)} preload="metadata" onPlay={() => setPlaying(true)} onPause={() => setPlaying(false)} onEnded={() => setPlaying(false)} onError={() => setError("Аудио недоступно. Импортируйте запись заново.")} onTimeUpdate={() => { const current = audio.current; if (!current) return; setPosition(current.currentTime); if (stopAt.current !== null && current.currentTime >= stopAt.current) { current.pause(); stopAt.current = null; } }} />
        <button className="meeting-play" disabled={busy} aria-label={playing ? "Пауза" : "Воспроизвести"} onClick={() => playing ? audio.current?.pause() : void play()}>{playing ? <Pause size={19} fill="currentColor" /> : <Play size={19} fill="currentColor" />}</button>
        <span className="meeting-time">{timecode(position)}</span><div className="meeting-timeline"><div className="meeting-voice-track" aria-hidden="true">{meeting.turns.map((turn, i) => <span key={i} style={{ left: `${turn.start / meeting.durationSeconds * 100}%`, width: `${Math.max(0.08, (turn.end - turn.start) / meeting.durationSeconds * 100)}%`, background: color(turn.speakerId) }} />)}</div><input type="range" aria-label="Позиция воспроизведения" min={0} max={meeting.durationSeconds} step={0.1} value={position} onChange={e => { const value = Number(e.target.value); setPosition(value); stopAt.current = null; if (audio.current) audio.current.currentTime = value; }} /></div><span className="meeting-time">{timecode(meeting.durationSeconds)}</span><select className="meeting-speed" aria-label="Скорость воспроизведения" onChange={e => { if (audio.current) audio.current.playbackRate = Number(e.target.value); }} defaultValue="1"><option value="0.75">0.75×</option><option value="1">1×</option><option value="1.25">1.25×</option><option value="1.5">1.5×</option><option value="2">2×</option></select>
      </section>
      {!meeting.segments.length && <section className="meeting-workflow">
        <div><div className="meeting-section-label"><Users size={16} /> Голоса участников</div><p>{meeting.speakers.length ? "Прослушайте образцы и назначьте каждому голосу имя." : "Найдём, где говорит каждый участник. Если знаете их количество, укажите его."}</p></div>
        <div className="meeting-workflow-actions"><label>Участников<select disabled={busy} aria-label="Количество участников" value={speakerCount} onChange={e => setSpeakerCount(Number(e.target.value))}><option value={0}>Авто</option>{Array.from({ length: 20 }, (_, i) => <option value={i + 1} key={i}>{i + 1}</option>)}</select></label><button className="meeting-button" disabled={busy || !tools?.diarizationReady} onClick={() => void task(async () => { await persist(); return analyzeMeeting(meeting.id, speakerCount); })}><AudioLines size={16} /> {meeting.speakers.length ? "Найти заново" : "Найти голоса"}</button></div>
      </section>}
      {meeting.speakers.length > 0 && <><div className="meeting-manual-tools"><span>Прослушайте голоса. Лишние группы можно объединить.</span><button className="meeting-button" disabled={busy || meeting.speakers.length >= 40} onClick={addSpeaker}><Plus size={14} /> Добавить участника</button></div><div className="meeting-speakers">{meeting.speakers.map((speaker, index) => <article className="meeting-speaker" key={speaker.id} style={{ "--speaker-color": colors[index % colors.length] } as React.CSSProperties}><div className="meeting-avatar">{speaker.name.trim().slice(0, 1).toUpperCase() || index + 1}</div><div><label htmlFor={`name-${speaker.id}`}>ГОЛОС {index + 1}</label><select aria-label={`Объединить ${speaker.name} с другим голосом`} value="" disabled={busy} onChange={e => mergeSpeaker(speaker.id, e.target.value)}><option value="">Объединить с…</option>{meeting.speakers.filter(s => s.id !== speaker.id).map(s => <option value={s.id} key={s.id}>{s.name}</option>)}</select><input id={`name-${speaker.id}`} value={speaker.name} disabled={busy} placeholder="Имя участника" onChange={e => update({ ...meeting, speakers: meeting.speakers.map(s => s.id === speaker.id ? { ...s, name: e.target.value } : s) })} /></div><button disabled={busy} title={`Прослушать ${speaker.name}`} aria-label={`Прослушать ${speaker.name}`} onClick={() => void play(speaker.sampleStart, speaker.sampleEnd)}><Headphones size={18} /></button></article>)}</div></>}
      {meeting.speakers.length > 0 && <VoiceCalibrationPanel key={`${meeting.id}-${meeting.calibration?.calibratedAt || "new"}-${meeting.speakers.map(s => s.id).join("-")}`} meeting={meeting} busy={busy} position={position} play={(start, end) => void play(start, end)} apply={refs => void task(async () => { await persist(); return calibrateMeeting(meeting.id, refs); }, "Голоса уточнены по вашим образцам. Неоднозначные реплики оставлены для проверки.")} />}
      {meeting.speakers.length > 0 && !meeting.segments.length && <section className="meeting-transcribe-action"><div><strong>Имена готовы? Сохраним разговор.</strong><p>Транскрипт получит таймкоды и имена участников.</p></div><select aria-label="Язык конференции" disabled={busy} value={language} onChange={e => setLanguage(e.target.value)}><option value="auto">Определить язык</option><option value="ru">Русский</option><option value="en">English</option><option value="uk">Українська</option><option value="zh">中文</option><option value="es">Español</option></select><button className="meeting-button primary" disabled={busy} onClick={() => void task(async () => { await persist(); const status = await getModelStatus(); if (!status.exists || !status.engineExists) throw new Error("Выберите и скачайте модель Whisper в настройках приложения."); return transcribeMeeting(meeting.id, language); })}><AudioLines size={17} /> Создать транскрипт</button></section>}
      {meeting.segments.length > 0 && meeting.calibration && <section className="meeting-transcribe-action"><div><strong>Точные границы. Имена по образцам.</strong><p>Определим смены голосов и перебивания, затем уточним слова по аудио. Результат появится отдельной версией; исходная запись и ручные назначения сохранятся.</p>{meeting.calibration.engine && <p>{meeting.calibration.engine} · {meeting.calibration.overlaps?.length || 0} участков возможной одновременной речи</p>}</div><select aria-label="Язык для уточнения" disabled={busy} value={language === "auto" ? "ru" : language} onChange={e => setLanguage(e.target.value)}><option value="ru">Русский</option><option value="en">English</option><option value="uk">Українська</option><option value="zh">中文</option><option value="es">Español</option></select><button className="meeting-button primary" disabled={busy} onClick={() => void task(async () => { await persist(); return refineMeeting(meeting.id, language === "auto" ? "ru" : language); }, "Готова уточнённая версия. Исходная конференция доступна в списке записей.")}>Уточнить реплики</button></section>}
      {meeting.segments.length > 0 && <section className="meeting-transcript">
        <header><div><h2>Транскрипт</h2><p>{meeting.segments.length} реплик · {meeting.modelName}{reviewCount > 0 ? ` · ${reviewCount} требуют проверки` : ""}</p></div><select aria-label="Формат экспорта" value={exportFormat} onChange={e => setExportFormat(e.target.value as typeof exportFormat)}><option value="txt">TXT</option><option value="srt">SRT</option><option value="json">JSON</option></select><button className="meeting-button" disabled={busy} onClick={() => void exportTranscript()}><Download size={16} /> Экспорт</button></header>
        <div className="meeting-transcript-tools"><label className="meeting-search"><Search size={16} /><input aria-label="Поиск в транскрипте" placeholder="Найти в разговоре…" value={search} onChange={e => setSearch(e.target.value)} /></label><select aria-label="Фильтр по участнику" value={filter} onChange={e => setFilter(e.target.value)}><option value="all">Все участники</option>{meeting.speakers.map(s => <option value={s.id} key={s.id}>{s.name}</option>)}<option value="unknown">Не определён</option></select><label className="meeting-review-filter"><input type="checkbox" checked={onlyReview} onChange={e => setOnlyReview(e.target.checked)} /> На проверку</label></div>
        <div className="meeting-segments">{segments.length === 0 ? <p className="meeting-no-results">По вашему запросу нет реплик.</p> : segments.map(segment => <article key={segment.id} className={`meeting-segment ${position >= segment.start && position < segment.end ? "is-playing" : ""}`} style={{ "--speaker-color": color(segment.speakerId) } as React.CSSProperties}><button className="meeting-segment-time" aria-label={`Воспроизвести реплику ${timecode(segment.start)}`} onClick={() => void play(segment.start, segment.end)}><Play size={12} />{timecode(segment.start)}</button><div className="meeting-segment-body"><div className="meeting-segment-meta"><span className="meeting-speaker-dot" /><select disabled={busy} aria-label={`Участник реплики ${timecode(segment.start)}`} value={segment.speakerId || ""} onChange={e => updateSegment(segment.id, { speakerId: e.target.value || null, needsReview: false, speakerLocked: true })}><option value="">Не определён</option>{meeting.speakers.map(s => <option key={s.id} value={s.id}>{s.name}</option>)}</select><button className="meeting-split" disabled={busy || position <= segment.start || position >= segment.end} title="Выберите момент внутри реплики на аудиодорожке и место в тексте" onClick={() => splitSegment(segment)}><Scissors size={12} /> Разделить</button>{segment.speakerLocked && <button className="meeting-review-badge" disabled={busy} title="Разрешить автоматическое обновление имени" onClick={() => updateSegment(segment.id, { speakerLocked: false })}>Имя закреплено · снять</button>}{segment.needsReview && <button className="meeting-review-badge" disabled={busy} onClick={() => updateSegment(segment.id, { needsReview: false })} title="Нажмите после проверки">Проверить ✓</button>}</div><textarea aria-label={`Текст реплики ${timecode(segment.start)}`} disabled={busy} value={segment.text} ref={element => { if (element) textAreas.current.set(segment.id, element); else textAreas.current.delete(segment.id); }} onChange={e => updateSegment(segment.id, { text: e.target.value })} rows={Math.max(1, Math.ceil(segment.text.length / 85))} /></div></article>)}</div>
      </section>}
    </>}
    <footer className="meeting-footer"><span><ShieldCheck size={14} /> Локальная обработка · только на вашем устройстве</span><button onClick={() => void openSettingsWindow().catch(e => setError(errorMessage(e)))}>Настройки моделей</button></footer>
  </div>;
}
