import { useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { open, save } from "@tauri-apps/plugin-dialog";
import { Download, FileAudio, Loader2, Plus, Sparkles, Trash2, X } from "lucide-react";
import { cancelQwen, deleteQwenVoice, exportQwenAudio, importQwenVoice, qwenSpeakers, qwenStatus, setupQwen, synthesizeQwen } from "../lib/qwen";
import type { QwenProgress, QwenStatus } from "../lib/qwen";
import { errorMessage, fileAssetUrl } from "../lib/tauri";
import "../styles/qwen.css";

export function QwenSpeech({ text, setText }: { text: string; setText: (value: string) => void }) {
  const [status, setStatus] = useState<QwenStatus | null>(null);
  const [progress, setProgress] = useState<QwenProgress | null>(null);
  const [voice, setVoice] = useState(() => localStorage.getItem("qwen_voice") || "Ryan");
  const [style, setStyle] = useState("Нейтрально, чётко и естественно.");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const [result, setResult] = useState<{ audioPath: string; voiceName: string } | null>(null);
  const [adding, setAdding] = useState(false);
  const [source, setSource] = useState("");
  const [name, setName] = useState("");
  const [referenceText, setReferenceText] = useState("");
  const pending = useRef(false);
  const audio = useRef<HTMLAudioElement>(null);
  const mounted = useRef(true);
  useEffect(() => { localStorage.setItem("qwen_voice", voice); }, [voice]);
  useEffect(() => {
    mounted.current = true;
    let disposed = false;
    const refresh = async () => {
      try {
        const next = await qwenStatus();
        if (disposed) return;
        setStatus(next);
        if (!pending.current) setBusy(next.busy);
        if (next.busy && next.progress) setProgress(next.progress);
        if (!next.busy && next.progress?.audioPath) setResult({ audioPath: next.progress.audioPath, voiceName: next.progress.voiceName || "Qwen" });
        if (!next.busy && next.progress?.error) setError(next.progress.message);
      } catch (e) { if (!disposed) setError(errorMessage(e)); }
    };
    const unlisten = window.__TAURI_INTERNALS__ ? listen<QwenProgress>("qwen-progress", event => { if (!disposed) setProgress(event.payload); }) : null;
    if (window.__TAURI_INTERNALS__) void refresh();
    const timer = window.setInterval(() => { if (window.__TAURI_INTERNALS__) void refresh(); }, 1800);
    return () => { disposed = true; mounted.current = false; window.clearInterval(timer); void unlisten?.then(fn => fn()); audio.current?.pause(); };
  }, []);

  async function task(action: () => Promise<void>) {
    if (busy || pending.current) return;
    pending.current = true; setBusy(true); setError(""); setProgress(null); audio.current?.pause();
    try {
      await action();
      const next = await qwenStatus();
      if (mounted.current) setStatus(next);
    } catch (e) { if (mounted.current) setError(errorMessage(e)); }
    finally { pending.current = false; if (mounted.current) setBusy(false); }
  }
  async function chooseSample() {
    const path = await open({ multiple: false, filters: [{ name: "Аудио", extensions: ["wav", "m4a", "mp3", "flac", "ogg"] }] });
    if (typeof path === "string") setSource(path);
  }
  const own = voice.startsWith("voice-");
  const exists = own ? status?.voices.some(item => item.id === voice) : qwenSpeakers.some(item => item.id === voice);

  return <div className="qwen-studio">
    <div className="qwen-heading">
      <div><span className="qwen-eyebrow"><Sparkles size={14} /> ЛОКАЛЬНАЯ СТУДИЯ ГОЛОСА</span><h2>Пусть текст звучит.</h2><p>Qwen3-TTS · русский язык · готовые голоса и ваш собственный.</p></div>
      <span className="qwen-chip">{status?.ready ? status.gpu || "NVIDIA CUDA" : "Qwen 1.7B"}</span>
    </div>
    {error && <div role="alert" className="error-banner">{error}</div>}
    {!status?.ready && <div className="qwen-install">
      <div><strong>Подготовьте Qwen один раз</strong><p>Два движка: готовые голоса и клонирование. Загрузка около 13 ГБ, на диске потребуется до 20 ГБ. После установки озвучивание работает без интернета.</p></div>
      <button className="primary-button" disabled={busy || !status} onClick={() => void task(setupQwen)}><Download size={16} /> Скачать Qwen</button>
    </div>}
    <div className="qwen-grid">
      <div className="qwen-editor">
        <label htmlFor="qwen-text">Текст для озвучивания</label>
        <textarea id="qwen-text" className="transcript-editor" value={text} maxLength={20000} disabled={busy} onChange={e => setText(e.target.value)} placeholder="Напишите текст. Например: Добро пожаловать! Сегодня мы расскажем о…" />
        <div className="qwen-actions"><span>{text.length.toLocaleString("ru-RU")} / 20 000</span><button className="primary-button" disabled={busy || !status?.ready || !text.trim() || !exists} onClick={() => void task(async () => { setResult(null); const next = await synthesizeQwen(text, voice, own ? "" : style); if (mounted.current) setResult(next); })}>{busy ? <Loader2 size={16} className="animate-spin" /> : <Sparkles size={16} />} Озвучить</button></div>
        {result && <div className="qwen-result"><div><strong>{result.voiceName}</strong><button className="secondary-button" onClick={async () => { try { const path = await save({ defaultPath: "озвучивание.wav", filters: [{ name: "WAV", extensions: ["wav"] }] }); if (path) await exportQwenAudio(result.audioPath, path); } catch (e) { setError(errorMessage(e)); } }}><Download size={14} /> Сохранить WAV</button></div><audio ref={audio} controls src={fileAssetUrl(result.audioPath)} /></div>}
      </div>
      <aside className="qwen-voices">
        <label htmlFor="qwen-voice">Голос</label>
        <select id="qwen-voice" value={voice} disabled={busy} onChange={e => setVoice(e.target.value)}>
          <optgroup label="Готовые голоса">{qwenSpeakers.map(item => <option value={item.id} key={item.id}>{item.name} — {item.description}</option>)}</optgroup>
          {!!status?.voices.length && <optgroup label="Мои голоса">{status.voices.map(item => <option key={item.id} value={item.id}>{item.name}</option>)}</optgroup>}
        </select>
        {!exists && status && <p className="qwen-note">Выберите доступный голос.</p>}
        <p className="qwen-note">{own ? "Озвучивание по вашему образцу: тембр и манера речи сохраняются." : "Все голоса умеют говорить по-русски. Для наиболее естественного произношения добавьте образец носителя языка."}</p>
        {!own && <><label htmlFor="qwen-style">Манера речи</label><select id="qwen-style" disabled={busy} value={style} onChange={e => setStyle(e.target.value)}>{["Нейтрально, чётко и естественно.", "Спокойно и тепло, как в личном разговоре.", "Уверенно, как профессиональный диктор.", "Радостно и энергично.", "Медленно, мягко и расслабленно."].map(item => <option key={item}>{item}</option>)}</select></>}
        <button className="secondary-button" disabled={busy} onClick={() => setAdding(!adding)}><Plus size={16} /> Клонировать голос</button>
        {own && <button className="secondary-button" disabled={busy} onClick={() => void task(async () => { await deleteQwenVoice(voice); setVoice("Ryan"); })}><Trash2 size={14} /> Удалить этот голос</button>}
      </aside>
    </div>
    {adding && <section className="qwen-clone"><div className="qwen-clone-heading"><h3>Ваш голос, новые слова.</h3><button className="icon-button" aria-label="Закрыть добавление голоса" disabled={busy} onClick={() => setAdding(false)}><X size={16} /></button></div><p>Добавьте 3–30 секунд чистой речи одного человека; лучше 10–30 секунд без музыки. Образец и его текст остаются на компьютере.</p><div className="qwen-clone-fields"><div><label htmlFor="qwen-name">Имя голоса</label><input id="qwen-name" value={name} maxLength={80} disabled={busy} onChange={e => setName(e.target.value)} placeholder="Например, мой голос" /><button className="secondary-button" disabled={busy} onClick={() => void chooseSample().catch(e => setError(errorMessage(e)))}><FileAudio size={16} /> {source ? source.split(/[\\/]/).pop() : "Выбрать аудиообразец"}</button></div><div><label htmlFor="qwen-reference">Точный текст из образца</label><textarea id="qwen-reference" maxLength={2000} value={referenceText} disabled={busy} onChange={e => setReferenceText(e.target.value)} placeholder="Напишите дословно всё, что произнесено в аудиообразце." /></div></div><button className="primary-button" disabled={busy || !source || !name.trim() || !referenceText.trim()} onClick={() => void task(async () => { const profile = await importQwenVoice(source, name, referenceText); if (mounted.current) { setVoice(profile.id); setAdding(false); setSource(""); setName(""); setReferenceText(""); } })}><Plus size={16} /> Сохранить голос</button></section>}
    {busy && <div className="qwen-progress" role="status"><div><Loader2 size={16} className="animate-spin" /><span>{progress?.message || "Подготовка…"}</span><button className="secondary-button" onClick={() => void cancelQwen().catch(e => setError(errorMessage(e)))}>Отменить</button></div><progress max={100} value={progress?.percentage || 0} /></div>}
  </div>;
}
