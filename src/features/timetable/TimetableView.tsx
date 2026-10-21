import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { useStore } from "../../app/providers/store";
import { useI18n } from "../../app/providers/i18n";
import { rpc } from "../../services/desktop-bridge/api";
import { growth } from "../../services/desktop-bridge/growthApi";
import type { Goal, Habit, Task } from "../../shared/types";
import { addDays, clsx } from "../../shared/utils";
import { Icon } from "../../components/ui/Icon";
import { Modal } from "../../components/ui/Modal";
import { api } from "../../services/desktop-bridge/api";

export type Kind = "study" | "work" | "class" | "habit" | "personal" | "other";
export interface Block {
  id: string;
  title: string;
  kind: Kind;
  recurrence: "once" | "weekly";
  date: string | null;
  weekdays: number[];
  startTime: string;
  endTime: string;
  validFrom: string | null;
  validUntil: string | null;
  taskId: string | null;
  goalId: string | null;
  habitId: string | null;
  location: string;
  notes: string;
  remindMinutes: number | null;
  taskTitle: string | null;
  goalTitle: string | null;
  habitTitle: string | null;
  occurrenceDate?: string;
  overlaps?: boolean;
}
interface Day {
  date: string;
  blocks: Block[];
  tasks: { id: string; title: string; status: string; priority: string; remindTime: string | null }[];
  sessions: { id: string; start: string; minutes: number; title: string }[];
}
interface Week {
  weekStart: string;
  today: string;
  days: Day[];
  plannedMinutes: number;
  actualMinutes: number;
  goals: { goalId: string; title: string; plannedMinutes: number; actualMinutes: number; budgetMinutes: number | null }[];
}

export const timetableApi = {
  week: (start?: string) => rpc<Week>("timetable.week", { start }),
  today: () => rpc<Block[]>("timetable.today"),
  create: (block: Record<string, unknown>) => rpc<Block>("timetable.create", { block }),
  update: (id: string, patch: Record<string, unknown>, date?: string) => rpc<Block>("timetable.update", { id, patch, date }),
  remove: (id: string, date?: string) => rpc("timetable.delete", { id, date }),
};

export const KINDS: { id: Kind; label: string; icon: string }[] = [
  { id: "study", label: "Study", icon: "📘" },
  { id: "work", label: "Work", icon: "💼" },
  { id: "class", label: "Class", icon: "🏫" },
  { id: "habit", label: "Habit", icon: "🔁" },
  { id: "personal", label: "Personal", icon: "🌱" },
  { id: "other", label: "Other", icon: "•" },
];
const kindColor = (k: string) => `var(--k-${k})`;
const toMin = (t: string) => {
  const [h, m] = t.split(":").map(Number);
  return h * 60 + m;
};
const toTime = (m: number) => `${String(Math.floor(m / 60)).padStart(2, "0")}:${String(m % 60).padStart(2, "0")}`;
const SNAP = 15;
const snap = (m: number) => Math.round(m / SNAP) * SNAP;
const fmtH = (m: number) => `${Math.floor(m / 60)}:${String(m % 60).padStart(2, "0")}`;

/** Lays overlapping blocks of one day side by side. */
function lanes(blocks: Block[]) {
  const out = new Map<Block, { lane: number; of: number }>();
  const sorted = [...blocks].sort((a, b) => toMin(a.startTime) - toMin(b.startTime));
  let group: Block[] = [];
  let groupEnd = -1;
  const flush = () => {
    const ends: number[] = [];
    for (const b of group) {
      let lane = ends.findIndex((e) => e <= toMin(b.startTime));
      if (lane === -1) lane = ends.push(0) - 1;
      ends[lane] = toMin(b.endTime);
      out.set(b, { lane, of: 0 });
    }
    group.forEach((b) => (out.get(b)!.of = ends.length));
    group = [];
  };
  for (const b of sorted) {
    if (toMin(b.startTime) >= groupEnd && group.length) flush();
    group.push(b);
    groupEnd = Math.max(groupEnd, toMin(b.endTime));
  }
  if (group.length) flush();
  return out;
}

type Drag =
  | { mode: "create"; day: number; from: number; to: number }
  | { mode: "move"; block: Block; day: number; offset: number; start: number; moved: boolean }
  | { mode: "resize"; block: Block; day: number; end: number };

export function TimetableView() {
  const s = useStore();
  const { t, locale } = useI18n();
  const [anchor, setAnchor] = useState<string | undefined>(undefined);
  const [week, setWeek] = useState<Week | null>(null);
  const [narrow, setNarrow] = useState(() => window.innerWidth < 900);
  const [dayIdx, setDayIdx] = useState(0);
  const [drag, setDrag] = useState<Drag | null>(null);
  const [editing, setEditing] = useState<{ block: Partial<Block>; occurrence?: string } | null>(null);
  const gridRef = useRef<HTMLDivElement>(null);
  const [now, setNow] = useState(new Date());

  const load = useCallback(() => timetableApi.week(anchor).then((w) => {
    setWeek(w);
    const ti = w.days.findIndex((d) => d.date === w.today);
    if (ti >= 0 && anchor === undefined) setDayIdx(ti);
  }).catch(s.fail), [anchor, s.fail]);
  useEffect(() => {
    load();
  }, [load, s.taskDialog]);
  useEffect(() => {
    const on = () => setNarrow(window.innerWidth < 900);
    window.addEventListener("resize", on);
    const timer = setInterval(() => setNow(new Date()), 60_000);
    return () => (window.removeEventListener("resize", on), clearInterval(timer));
  }, []);

  // Visible hours: 06–23 by default, widened to fit any block.
  const [startH, endH] = useMemo(() => {
    let a = 6, z = 23;
    week?.days.forEach((d) => d.blocks.forEach((b) => { a = Math.min(a, Math.floor(toMin(b.startTime) / 60)); z = Math.max(z, Math.ceil(toMin(b.endTime) / 60)); }));
    return [a, Math.min(24, z)];
  }, [week]);
  const HOUR = 48;
  const pxMin = HOUR / 60;
  const days = week ? (narrow ? [week.days[dayIdx]] : week.days) : [];
  const dayOffset = narrow ? dayIdx : 0;

  const minuteAt = (clientY: number, col: HTMLElement) => {
    const r = col.getBoundingClientRect();
    return Math.max(startH * 60, Math.min(endH * 60, snap((clientY - r.top) / pxMin + startH * 60)));
  };
  const dayAt = (clientX: number) => {
    const cols = gridRef.current?.querySelectorAll<HTMLElement>(".tt-col") ?? [];
    let idx = 0;
    cols.forEach((c, i) => { const r = c.getBoundingClientRect(); if (clientX >= r.left) idx = i; });
    return idx;
  };
  const colEl = (i: number) => gridRef.current?.querySelectorAll<HTMLElement>(".tt-col")[i];

  const onColDown = (e: React.PointerEvent, i: number) => {
    if (e.button !== 0 || (e.target as HTMLElement).closest(".tt-block, .tt-marker, .tt-session")) return;
    const m = minuteAt(e.clientY, e.currentTarget as HTMLElement);
    (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
    setDrag({ mode: "create", day: i, from: m, to: m + 60 });
  };
  const onMove = (e: React.PointerEvent) => {
    if (!drag) return;
    if (drag.mode === "create") {
      const col = colEl(drag.day);
      if (col) setDrag({ ...drag, to: Math.max(drag.from + SNAP, minuteAt(e.clientY, col)) });
    } else if (drag.mode === "move") {
      const day = dayAt(e.clientX);
      const col = colEl(day);
      if (!col) return;
      const len = toMin(drag.block.endTime) - toMin(drag.block.startTime);
      const start = Math.max(startH * 60, Math.min(endH * 60 - len, snap(minuteAt(e.clientY, col) - drag.offset)));
      if (start !== drag.start || day !== drag.day) setDrag({ ...drag, day, start, moved: true });
    } else {
      const col = colEl(drag.day);
      if (col) setDrag({ ...drag, end: Math.max(toMin(drag.block.startTime) + SNAP, minuteAt(e.clientY, col)) });
    }
  };
  const onUp = async () => {
    const d = drag;
    setDrag(null);
    if (!d || !week) return;
    if (d.mode === "create") {
      const date = week.days[d.day + dayOffset].date;
      setEditing({ block: { title: "", kind: "study", recurrence: "once", date, startTime: toTime(d.from), endTime: toTime(Math.min(24 * 60 - 1, d.to)), weekdays: [isoDay(date)] } });
      return;
    }
    const b = d.block;
    const occ = b.occurrenceDate!;
    let patch: Record<string, unknown>;
    if (d.mode === "move") {
      if (!d.moved) return setEditing({ block: b, occurrence: occ });
      const len = toMin(b.endTime) - toMin(b.startTime);
      const newDate = week.days[d.day + dayOffset].date;
      patch = { startTime: toTime(d.start), endTime: toTime(d.start + len) };
      if (b.recurrence === "once") patch.date = newDate;
      else if (newDate !== occ) {
        // Moving a weekly block to another day changes only that occurrence.
        return timetableApi.update(b.id, { ...patch, date: newDate }, occ).then(load).catch(s.fail);
      }
    } else {
      patch = { endTime: toTime(Math.min(24 * 60 - 1, d.end)) };
    }
    if (b.recurrence === "weekly") {
      const all = await s.confirm({ title: t("Change every week?"), message: t("“{name}” repeats weekly. Choose OK to change every week, or Cancel to change only {date}.", { name: b.title, date: occ }), confirmLabel: t("Every week") });
      return timetableApi.update(b.id, patch, all ? undefined : occ).then(load).catch(s.fail);
    }
    timetableApi.update(b.id, patch).then(load).catch(s.fail);
  };

  if (!week) return <div className="content"><div className="loading">…</div></div>;
  const todayIdx = week.days.findIndex((d) => d.date === week.today);
  const nowMin = now.getHours() * 60 + now.getMinutes();
  const range = `${new Date(week.weekStart + "T00:00:00Z").toLocaleDateString(locale, { day: "numeric", month: "short", timeZone: "UTC" })} – ${new Date(addDays(week.weekStart, 6) + "T00:00:00Z").toLocaleDateString(locale, { day: "numeric", month: "short", year: "numeric", timeZone: "UTC" })}`;

  return (
    <div className="content">
      <div className="page full" style={{ maxWidth: 1400, margin: "0 auto" }}>
        <div className="row wrap" style={{ marginTop: 8 }}>
          <h1 style={{ margin: 0, fontSize: 28 }}>📅 {t("Timetable")}</h1>
          <span className="spacer" />
          <button className="icon-btn" aria-label={t("Previous week")} onClick={() => (narrow ? (dayIdx > 0 ? setDayIdx(dayIdx - 1) : (setAnchor(addDays(week.weekStart, -7)), setDayIdx(6))) : setAnchor(addDays(week.weekStart, -7)))}><Icon name="chevronLeft" /></button>
          <button className="btn small" onClick={() => (setAnchor(undefined), setDayIdx(todayIdx >= 0 ? todayIdx : 0))}>{t("Today")}</button>
          <button className="icon-btn" aria-label={t("Next week")} onClick={() => (narrow ? (dayIdx < 6 ? setDayIdx(dayIdx + 1) : (setAnchor(addDays(week.weekStart, 7)), setDayIdx(0))) : setAnchor(addDays(week.weekStart, 7)))}><Icon name="chevronRight" /></button>
          <strong style={{ minWidth: 190, textAlign: "center" }}>{range}</strong>
          <button className="btn primary" onClick={() => setEditing({ block: { title: "", kind: "study", recurrence: "weekly", weekdays: [1, 2, 3, 4, 5], startTime: "18:00", endTime: "19:00", date: week.today } })}>＋ {t("Block")}</button>
        </div>
        <div className="row wrap small-text muted" style={{ margin: "8px 0 10px", gap: 14 }}>
          <span>{t("Planned")} <strong style={{ color: "var(--text)" }}>{fmtH(week.plannedMinutes)} h</strong></span>
          <span>{t("Focused (actual)")} <strong style={{ color: "var(--text)" }}>{fmtH(week.actualMinutes)} h</strong></span>
          {week.goals.map((g) => (
            <span key={g.goalId} className="chip" title={t("planned / weekly budget · actual focus")} style={{ cursor: "default" }} onClick={() => s.navigate({ name: "goal", id: g.goalId })}>
              🎯 {g.title}: {fmtH(g.plannedMinutes)}{g.budgetMinutes ? ` / ${fmtH(g.budgetMinutes)}` : ""} h · {t("actual")} {fmtH(g.actualMinutes)} h
              {g.budgetMinutes && g.plannedMinutes < g.budgetMinutes ? " ⚠︎" : ""}
            </span>
          ))}
          <span className="spacer" />
          <span className="tt-legend">{KINDS.map((k) => <span key={k.id} style={{ ["--kc" as string]: kindColor(k.id) }}>{t(k.label)}</span>)}</span>
        </div>

        <div className="tt" ref={gridRef} style={{ ["--days" as string]: days.length, ["--hour" as string]: `${HOUR}px` }} onPointerMove={onMove} onPointerUp={onUp} role="grid" aria-label={t("Timetable")}>
          <div className="tt-head" />
          {days.map((d, i) => {
            const dt = new Date(d.date + "T00:00:00Z");
            return (
              <div key={d.date} className={clsx("tt-head", d.date === week.today && "today")} role="columnheader">
                {dt.toLocaleDateString(locale, { weekday: "short", timeZone: "UTC" })}
                <span className="num">{dt.getUTCDate()}</span>
                <span className="faint">{fmtH(d.blocks.reduce((m, b) => m + toMin(b.endTime) - toMin(b.startTime), 0))} h</span>
                {i === 0 && null}
              </div>
            );
          })}
          <div className="small-text faint" style={{ padding: "6px 4px", fontSize: 10, textAlign: "right" }}>{t("Due")}</div>
          {days.map((d) => (
            <div key={d.date} className="tt-allday">
              {d.tasks.filter((x) => !x.remindTime).map((x) => (
                <span key={x.id} className="chip" onClick={() => s.openTask(x.id)} role="button" title={x.title} style={{ textDecoration: x.status === "done" ? "line-through" : undefined }}>
                  {x.status === "done" ? "✓" : "○"} {x.title}
                </span>
              ))}
            </div>
          ))}
          <div className="tt-hours">
            {Array.from({ length: endH - startH }, (_, i) => <div key={i} className="tt-hour">{i === 0 ? "" : `${String(startH + i).padStart(2, "0")}:00`}</div>)}
          </div>
          {days.map((d, i) => {
            const lay = lanes(d.blocks);
            const isToday = d.date === week.today;
            return (
              <div key={d.date} className={clsx("tt-col", isToday && "today")} style={{ height: (endH - startH) * HOUR }} onPointerDown={(e) => onColDown(e, i)} role="gridcell" aria-label={d.date}>
                {d.blocks.map((b) => {
                  const draggingThis = drag && drag.mode !== "create" && drag.block.id === b.id && drag.block.occurrenceDate === b.occurrenceDate;
                  if (draggingThis && drag.mode === "move" && drag.day !== i) return null;
                  let s0 = toMin(b.startTime), e0 = toMin(b.endTime);
                  if (draggingThis && drag.mode === "move") (e0 = drag.start + (e0 - s0)), (s0 = drag.start);
                  if (draggingThis && drag.mode === "resize") e0 = drag.end;
                  const l = lay.get(b) ?? { lane: 0, of: 1 };
                  const kind = KINDS.find((k) => k.id === b.kind);
                  return (
                    <button
                      key={b.id + (b.occurrenceDate ?? "")}
                      className={clsx("tt-block", b.overlaps && "clash")}
                      style={{ top: (s0 - startH * 60) * pxMin, height: Math.max(18, (e0 - s0) * pxMin - 2), left: `calc(${(l.lane / l.of) * 100}% + 2px)`, width: `calc(${100 / l.of}% - 4px)`, right: "auto", ["--kc" as string]: kindColor(b.kind) }}
                      onPointerDown={(e) => {
                        if ((e.target as HTMLElement).classList.contains("resize")) return;
                        e.stopPropagation();
                        const col = colEl(i)!;
                        gridRef.current?.setPointerCapture(e.pointerId);
                        setDrag({ mode: "move", block: b, day: i, offset: minuteAt(e.clientY, col) - toMin(b.startTime), start: toMin(b.startTime), moved: false });
                      }}
                      onKeyDown={(e) => e.key === "Enter" && setEditing({ block: b, occurrence: b.occurrenceDate })}
                      aria-label={`${b.title}, ${b.startTime}–${b.endTime}${b.recurrence === "weekly" ? `, ${t("weekly")}` : ""}`}
                      title={`${b.title}\n${toTime(s0)}–${toTime(e0)}${b.location ? ` · ${b.location}` : ""}${b.overlaps ? `\n⚠ ${t("Overlaps another block")}` : ""}`}
                    >
                      <span className="t">{b.recurrence === "weekly" ? "↻ " : ""}{b.title}</span>
                      <span className="m">{toTime(s0)}–{toTime(e0)} · {kind ? t(kind.label) : ""}{b.goalTitle ? ` · 🎯 ${b.goalTitle}` : ""}{b.location ? ` · ${b.location}` : ""}</span>
                      <span className="resize" onPointerDown={(e) => {
                        e.stopPropagation();
                        gridRef.current?.setPointerCapture(e.pointerId);
                        setDrag({ mode: "resize", block: b, day: i, end: toMin(b.endTime) });
                      }} />
                    </button>
                  );
                })}
                {drag && drag.mode === "move" && drag.day === i && !d.blocks.some((b) => b.id === drag.block.id && b.occurrenceDate === drag.block.occurrenceDate) && (
                  <div className="tt-block ghost" style={{ top: (drag.start - startH * 60) * pxMin, height: (toMin(drag.block.endTime) - toMin(drag.block.startTime)) * pxMin, ["--kc" as string]: kindColor(drag.block.kind) }}>
                    <span className="t">{drag.block.title}</span>
                  </div>
                )}
                {drag?.mode === "create" && drag.day === i && (
                  <div className="tt-block ghost" style={{ top: (drag.from - startH * 60) * pxMin, height: (drag.to - drag.from) * pxMin, ["--kc" as string]: kindColor("study") }}>
                    <span className="t">{toTime(drag.from)}–{toTime(drag.to)}</span>
                  </div>
                )}
                {d.sessions.map((x) => toMin(x.start) >= startH * 60 && (
                  <div key={x.id} className="tt-session" style={{ top: (toMin(x.start) - startH * 60) * pxMin, height: Math.max(4, x.minutes * pxMin) }} title={`${t("Focused")}: ${x.title} · ${x.minutes} min`} />
                ))}
                {d.tasks.filter((x) => x.remindTime).map((x) => (
                  <div key={x.id} className="tt-marker" style={{ top: (toMin(x.remindTime!) - startH * 60) * pxMin - 7 }} onClick={() => s.openTask(x.id)} role="button" title={x.title}>⏰ {x.title}</div>
                ))}
                {isToday && nowMin >= startH * 60 && nowMin <= endH * 60 && <div className="tt-now" style={{ top: (nowMin - startH * 60) * pxMin }} aria-hidden />}
              </div>
            );
          })}
        </div>
        <p className="small-text faint" style={{ marginTop: 8 }}>
          {t("Drag on an empty slot to add a block · drag a block to move it · drag its bottom edge to resize · ↻ repeats weekly · striped bars are focus time you actually logged.")}
        </p>
      </div>
      {editing && <BlockDialog init={editing.block} occurrence={editing.occurrence} onClose={() => setEditing(null)} onSaved={() => (setEditing(null), load())} />}
    </div>
  );
}

const isoDay = (date: string) => ((new Date(date + "T00:00:00Z").getUTCDay() + 6) % 7) + 1;

export function BlockDialog({ init, occurrence, onClose, onSaved }: { init: Partial<Block>; occurrence?: string; onClose: () => void; onSaved: () => void }) {
  const s = useStore();
  const { t, locale } = useI18n();
  const isNew = !init.id;
  const [b, setB] = useState<Partial<Block>>({ ...init, date: init.recurrence === "once" ? init.date : occurrence ?? init.date });
  const [onlyThis, setOnlyThis] = useState(false);
  const [goals, setGoals] = useState<Goal[]>([]);
  const [tasks, setTasks] = useState<Task[]>([]);
  const [habits, setHabits] = useState<Habit[]>([]);
  useEffect(() => {
    growth.goals.list().then(setGoals).catch(() => {});
    api.tasks.list("open", { limit: 300 }).then(setTasks).catch(() => {});
    growth.habits.list().then(setHabits).catch(() => {});
  }, []);
  const set = (patch: Partial<Block>) => setB({ ...b, ...patch });
  const body = () => ({
    title: b.title, kind: b.kind, recurrence: b.recurrence, date: b.date, weekdays: b.weekdays, startTime: b.startTime, endTime: b.endTime,
    validFrom: b.validFrom || null, validUntil: b.validUntil || null, goalId: b.goalId || null, taskId: b.taskId || null, habitId: b.habitId || null,
    location: b.location ?? "", notes: b.notes ?? "", remindMinutes: b.remindMinutes ?? null,
  });
  const save = () => {
    if (!b.title?.trim()) return s.toast(t("Give the block a name."), { kind: "error" });
    const req = isNew ? timetableApi.create(body()) : timetableApi.update(init.id!, body(), onlyThis && occurrence ? occurrence : undefined);
    req.then(onSaved).catch(s.fail);
  };
  const remove = async (all: boolean) => {
    if (!init.id) return;
    await timetableApi.remove(init.id, all ? undefined : occurrence).catch(s.fail);
    onSaved();
  };
  const weekly = b.recurrence === "weekly";
  return (
    <Modal title={isNew ? t("New block") : t("Edit block")} wide onClose={onClose} footer={<>
      {!isNew && (weekly && occurrence
        ? <><button className="btn danger" onClick={() => remove(false)}>{t("Delete this day")}</button><button className="btn danger" onClick={() => remove(true)}>{t("Delete series")}</button></>
        : <button className="btn danger" onClick={() => remove(true)}>{t("Delete")}</button>)}
      <span className="spacer" />
      <button className="btn" onClick={onClose}>{t("Cancel")}</button>
      <button className="btn primary" onClick={save}>{t("Save")}</button>
    </>}>
      <input className="input" autoFocus style={{ fontSize: 17, height: 40, fontWeight: 500 }} placeholder={t("What will you do? e.g. Maths practice")} value={b.title ?? ""} onChange={(e) => set({ title: e.target.value })} aria-label={t("Title")} />
      <div className="row wrap" style={{ margin: "10px 0" }}>
        {KINDS.map((k) => (
          <button key={k.id} type="button" className={`btn small${b.kind === k.id ? " primary" : ""}`} aria-pressed={b.kind === k.id} onClick={() => set({ kind: k.id })}>{k.icon} {t(k.label)}</button>
        ))}
      </div>
      <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr 1fr", gap: "0 12px" }}>
        <div className="field"><label>{t("Start")}</label><input className="input" type="time" step={900} value={b.startTime ?? ""} onChange={(e) => set({ startTime: e.target.value })} /></div>
        <div className="field"><label>{t("End")}</label><input className="input" type="time" step={900} value={b.endTime ?? ""} onChange={(e) => set({ endTime: e.target.value })} /></div>
        <div className="field"><label>{t("Repeat")}</label>
          <select className="select" value={b.recurrence ?? "once"} onChange={(e) => set({ recurrence: e.target.value as Block["recurrence"] })} disabled={!isNew && onlyThis}>
            <option value="once">{t("Once")}</option><option value="weekly">{t("Every week")}</option>
          </select>
        </div>
      </div>
      {weekly ? (
        <>
          <div className="row" style={{ gap: 4, marginBottom: 10 }}>
            {[1, 2, 3, 4, 5, 6, 7].map((d) => (
              <button key={d} type="button" className={`btn small${b.weekdays?.includes(d) ? " primary" : ""}`} aria-pressed={!!b.weekdays?.includes(d)}
                onClick={() => set({ weekdays: b.weekdays?.includes(d) ? b.weekdays.filter((x) => x !== d) : [...(b.weekdays ?? []), d].sort() })}>
                {new Date(Date.UTC(2024, 0, d)).toLocaleDateString(locale, { weekday: "short", timeZone: "UTC" })}
              </button>
            ))}
          </div>
          <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: "0 12px" }}>
            <div className="field"><label>{t("From (optional)")}</label><input className="input" type="date" value={b.validFrom ?? ""} onChange={(e) => set({ validFrom: e.target.value })} /></div>
            <div className="field"><label>{t("Until (optional, e.g. end of term)")}</label><input className="input" type="date" value={b.validUntil ?? ""} onChange={(e) => set({ validUntil: e.target.value })} /></div>
          </div>
          {!isNew && occurrence && <label className="row small-text" style={{ marginBottom: 10 }}><input type="checkbox" checked={onlyThis} onChange={(e) => setOnlyThis(e.target.checked)} /> {t("Change only {date}", { date: occurrence })}</label>}
        </>
      ) : (
        <div className="field"><label>{t("Date")}</label><input className="input" type="date" style={{ width: 200 }} value={b.date ?? ""} onChange={(e) => set({ date: e.target.value })} /></div>
      )}
      <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr 1fr", gap: "0 12px" }}>
        <div className="field"><label>{t("For goal")}</label><select className="select" value={b.goalId ?? ""} onChange={(e) => set({ goalId: e.target.value })}><option value="">—</option>{goals.map((g) => <option key={g.id} value={g.id}>{g.title}</option>)}</select></div>
        <div className="field"><label>{t("Task")}</label><select className="select" value={b.taskId ?? ""} onChange={(e) => set({ taskId: e.target.value })}><option value="">—</option>{tasks.map((x) => <option key={x.id} value={x.id}>{x.title}</option>)}</select></div>
        <div className="field"><label>{t("Habit")}</label><select className="select" value={b.habitId ?? ""} onChange={(e) => set({ habitId: e.target.value })}><option value="">—</option>{habits.map((h) => <option key={h.id} value={h.id}>{h.title}</option>)}</select></div>
        <div className="field"><label>{t("Place")}</label><input className="input" value={b.location ?? ""} placeholder={t("e.g. Library, Room 301")} onChange={(e) => set({ location: e.target.value })} /></div>
        <div className="field"><label>{t("Remind me")}</label>
          <select className="select" value={b.remindMinutes ?? ""} onChange={(e) => set({ remindMinutes: e.target.value === "" ? null : Number(e.target.value) })}>
            <option value="">{t("No reminder")}</option><option value="0">{t("At start")}</option>
            {[5, 10, 15, 30, 60].map((m) => <option key={m} value={m}>{t("{m} min before", { m })}</option>)}
          </select>
        </div>
      </div>
      <div className="field"><label>{t("Notes")}</label><textarea className="textarea" rows={2} value={b.notes ?? ""} onChange={(e) => set({ notes: e.target.value })} /></div>
    </Modal>
  );
}

/** Compact "today's schedule" for Home. */
export function TodaySchedule({ blocks, onChanged }: { blocks: Block[]; onChanged?: () => void }) {
  const s = useStore();
  const { t } = useI18n();
  const [editing, setEditing] = useState<Block | null>(null);
  const nowMin = new Date().getHours() * 60 + new Date().getMinutes();
  if (!blocks.length) return <div className="muted small-text">{t("Nothing planned today.")} <button className="crumb" onClick={() => s.navigate({ name: "timetable" })}>{t("Open timetable")} →</button></div>;
  return (
    <div>
      {blocks.map((b) => {
        const live = toMin(b.startTime) <= nowMin && nowMin < toMin(b.endTime);
        const past = toMin(b.endTime) <= nowMin;
        return (
          <div key={b.id} className="list-row" style={{ opacity: past ? 0.55 : 1 }} onClick={() => setEditing(b)} role="button" tabIndex={0}>
            <span style={{ width: 92, fontVariantNumeric: "tabular-nums" }} className="small-text">{b.startTime}–{b.endTime}</span>
            <span style={{ width: 4, alignSelf: "stretch", borderRadius: 2, background: kindColor(b.kind) }} aria-hidden />
            <span className="t">{b.title}{b.location && <span className="faint small-text"> · {b.location}</span>}</span>
            {live && <span className="chip c-red">● {t("Now")}</span>}
            {b.goalTitle && <span className="small-text muted">🎯 {b.goalTitle}</span>}
          </div>
        );
      })}
      {editing && <BlockDialog init={editing} occurrence={editing.occurrenceDate} onClose={() => setEditing(null)} onSaved={() => (setEditing(null), onChanged?.())} />}
    </div>
  );
}
