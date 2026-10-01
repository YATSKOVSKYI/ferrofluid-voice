import { useState } from "react";
import { AudioLines, Headphones, Plus, X } from "lucide-react";
import { timecode } from "../lib/meetings";
import type { Meeting, VoiceReference } from "../lib/meetings";

function seconds(value: string) {
  const parts = value.trim().split(":");
  if (parts.length < 2 || parts.length > 3 || parts.some(p => !/^\d{1,3}$/.test(p))) return NaN;
  const numbers = parts.map(Number);
  if (numbers.slice(1).some(n => n >= 60)) return NaN;
  return numbers.reduce((total, n) => total * 60 + n, 0);
}
export function VoiceCalibrationPanel({ meeting, busy, position, play, apply }: {
  meeting: Meeting; busy: boolean; position: number;
  play: (start: number, end: number) => void; apply: (refs: VoiceReference[]) => void;
}) {
  const [rows, setRows] = useState(() => {
    const confirmed = (meeting.calibration?.references || []).filter(r => meeting.speakers.some(s => s.id === r.speakerId)).map(r => ({ ...r, from: timecode(r.start), to: timecode(r.end) }));
    return [...confirmed, ...meeting.speakers.filter(s => !confirmed.some(r => r.speakerId === s.id)).map(s => ({ speakerId: s.id, name: s.name, start: 0, end: 0, from: "", to: "" }))];
  });
  const [error, setError] = useState("");
  const edit = (index: number, patch: Partial<typeof rows[number]>) => { setRows(current => current.map((r, i) => i === index ? { ...r, ...patch } : r)); setError(""); };
  const parsed = () => rows.map(r => ({ speakerId: r.speakerId, name: meeting.speakers.find(s => s.id === r.speakerId)?.name.trim() || "", start: seconds(r.from), end: seconds(r.to) }));
  function submit() {
    const refs = parsed();
    if (meeting.speakers.some(s => !refs.some(r => r.speakerId === s.id))) { setError("Добавьте образец для каждого участника."); return; }
    if (refs.some(r => !r.name || !Number.isFinite(r.start) || !Number.isFinite(r.end) || r.end - r.start < 7 || r.end - r.start > 120 || r.end > meeting.durationSeconds)) { setError("Укажите имя и интервал чистой речи от 7 до 120 секунд, например 03:14 — 03:43."); return; }
    if (refs.some((r, i) => refs.slice(i + 1).some(q => q.speakerId !== r.speakerId && Math.max(q.start, r.start) < Math.min(q.end, r.end)))) { setError("Образцы разных участников не должны пересекаться."); return; }
    apply(refs);
  }
  return <section className="meeting-calibration">
    <div className="meeting-section-label"><AudioLines size={17} /> Подтверждённые образцы голосов</div>
    <p>Для каждого участника выберите отрывок, где говорит только он. По этим образцам повторно определим голоса во всей записи. Текст и закреплённые вручную имена сохранятся.</p>
    {rows.map((row, i) => <div className="meeting-calibration-row" key={i}>
      <select aria-label={`Участник образца ${i + 1}`} disabled={busy} value={row.speakerId} onChange={e => edit(i, { speakerId: e.target.value })}>{meeting.speakers.map(s => <option value={s.id} key={s.id}>{s.name}</option>)}</select>
      <input aria-label={`Начало образца ${i + 1}`} placeholder="03:14" disabled={busy} value={row.from} onChange={e => edit(i, { from: e.target.value })} /> <span>—</span>
      <input aria-label={`Конец образца ${i + 1}`} placeholder="03:43" disabled={busy} value={row.to} onChange={e => edit(i, { to: e.target.value })} />
      <button className="meeting-button" disabled={busy} onClick={() => edit(i, { from: timecode(Math.floor(position)), to: timecode(Math.min(Math.floor(position) + 10, Math.floor(meeting.durationSeconds))) })}>От курсора</button>
      <button className="meeting-button" aria-label={`Прослушать образец ${i + 1}`} disabled={busy || !Number.isFinite(seconds(row.from)) || seconds(row.to) <= seconds(row.from)} onClick={() => play(seconds(row.from), seconds(row.to))}><Headphones size={16} /></button>
      <button className="meeting-button" aria-label={`Удалить образец ${i + 1}`} disabled={busy} onClick={() => setRows(rows.filter((_, index) => i !== index))}><X size={14} /></button>
    </div>)}
    {error && <p role="alert" className="meeting-calibration-error">{error}</p>}
    <div className="meeting-calibration-actions"><button className="meeting-button" disabled={busy || rows.length >= 80} onClick={() => setRows([...rows, { speakerId: meeting.speakers[0].id, name: "", start: 0, end: 0, from: "", to: "" }])}><Plus size={15} /> Ещё образец</button><button className="meeting-button primary" disabled={busy} onClick={submit}>Уточнить голоса</button></div>
    <small>{meeting.calibration ? "Образцы применены. Неоднозначные реплики помечены для проверки." : "При первом запуске загрузится модель CAM++ (~27 МБ). Дальше обработка локальная."}</small>
  </section>;
}
