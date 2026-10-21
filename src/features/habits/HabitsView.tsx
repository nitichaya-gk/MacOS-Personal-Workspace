import { useCallback, useEffect, useState } from "react";
import { useStore } from "../../app/providers/store";
import { useI18n } from "../../app/providers/i18n";
import { growth, pct } from "../../services/desktop-bridge/growthApi";
import type { Goal, Habit, HabitSchedule, Skill } from "../../shared/types";
import { Icon } from "../../components/ui/Icon";
import { EmptyState, Modal } from "../../components/ui/Modal";

const STATUS_GLYPH: Record<string, string> = { done: "✓", skipped: "–", missed: "✕", open: "", not_due: "·" };
/** Unlogged → done → skipped (rest day) → cleared. */
const nextStatus = (d: { status: string; logged: boolean }): "done" | "skipped" | null =>
  !d.logged ? "done" : d.status === "done" ? "skipped" : d.status === "skipped" ? null : "done";

export function scheduleText(s: HabitSchedule, t: (x: string, v?: Record<string, string | number>) => string, locale: string) {
  if (s.type === "daily") return t("Every day");
  if (s.type === "weekly") return t("{n}× per week", { n: s.perWeek });
  const names = s.days.map((d) => new Date(Date.UTC(2024, 0, d)).toLocaleDateString(locale, { weekday: "short", timeZone: "UTC" }));
  return names.join(", ");
}

/** One habit row with the 14-day grid. Clicking a day cycles: done → skipped → clear. */
export function HabitRow({ h, onChanged, compact }: { h: Habit; onChanged: (h: Habit) => void; compact?: boolean }) {
  const s = useStore();
  const { t, locale } = useI18n();
  const days = compact ? h.recent.slice(-1) : h.recent;
  return (
    <div className="task-row" style={{ cursor: "default" }}>
      <div className="t">
        <div className="name">{h.title}</div>
        <div className="task-meta">
          <span>{scheduleText(h.schedule, t, locale)}</span>
          <span>🔥 {h.stats.streak} {t(h.stats.streakUnit)}</span>
          {h.stats.weekTarget ? <span>{h.stats.weekDone}/{h.stats.weekTarget} {t("this week")}</span> : <span>{t("7-day consistency")} {pct(h.stats.consistency7)}</span>}
          {h.goalTitle && <span>🎯 {h.goalTitle}</span>}
          {h.reminderTime && <span>⏰ {h.reminderTime}</span>}
        </div>
      </div>
      <div className="row" style={{ gap: 3 }}>
        {days.map((d) => {
          const isToday = d.date === h.recent[h.recent.length - 1].date;
          const label = `${d.date}: ${t(d.status)}`;
          return (
            <button
              key={d.date}
              title={label}
              aria-label={label}
              className="check"
              style={{
                borderRadius: 5, width: compact ? 26 : 22, height: compact ? 26 : 22, fontSize: 12,
                background: d.status === "done" ? "var(--success)" : d.status === "skipped" ? "var(--bg-active)" : "transparent",
                color: d.status === "done" ? "white" : d.status === "missed" && d.logged ? "var(--danger)" : "var(--text-muted)",
                borderColor: isToday ? "var(--accent)" : d.status === "not_due" ? "transparent" : undefined,
                opacity: d.status === "not_due" ? 0.5 : 1,
              }}
              onClick={() => growth.habits.log(h.id, d.date, nextStatus(d)).then(onChanged).catch(s.fail)}
            >
              {STATUS_GLYPH[d.status]}
            </button>
          );
        })}
      </div>
    </div>
  );
}

export function HabitsView() {
  const s = useStore();
  const { t } = useI18n();
  const [habits, setHabits] = useState<Habit[] | null>(null);
  const [editing, setEditing] = useState<Habit | "new" | null>(null);
  const load = useCallback(() => growth.habits.list().then(setHabits).catch(s.fail), [s.fail]);
  useEffect(() => {
    load();
  }, [load]);
  const replace = (h: Habit) => setHabits((list) => list?.map((x) => (x.id === h.id ? h : x)) ?? null);
  return (
    <div className="content">
      <div className="page">
        <div className="row" style={{ marginTop: 16 }}>
          <h1 style={{ margin: 0, fontSize: 30 }}>🔁 {t("Habits & routines")}</h1>
          <span className="spacer" />
          <button className="btn primary" onClick={() => setEditing("new")}>＋ {t("New habit")}</button>
        </div>
        <p className="muted small-text">{t("Habits measure the process, not the outcome. Skipped (rest) days don't break a streak; missed days do. Click a day to cycle done → skipped → clear.")}</p>
        {habits === null ? <div className="loading">…</div> : habits.length === 0 ? (
          <EmptyState icon="🔁" title={t("No habits yet")} hint={t("e.g. Review vocabulary 20 minutes, 5 days a week.")}><button className="btn primary" onClick={() => setEditing("new")}>＋ {t("New habit")}</button></EmptyState>
        ) : habits.map((h) => (
          <div key={h.id} className="row" style={{ alignItems: "center" }}>
            <div style={{ flex: 1 }}><HabitRow h={h} onChanged={replace} /></div>
            <button className="icon-btn small" aria-label={t("Edit habit")} onClick={() => setEditing(h)}><Icon name="more" size={14} /></button>
          </div>
        ))}
      </div>
      {editing && <HabitDialog habit={editing === "new" ? null : editing} onClose={() => setEditing(null)} onSaved={() => (setEditing(null), load())} />}
    </div>
  );
}

function HabitDialog({ habit, onClose, onSaved }: { habit: Habit | null; onClose: () => void; onSaved: () => void }) {
  const s = useStore();
  const { t, locale } = useI18n();
  const [goals, setGoals] = useState<Goal[]>([]);
  const [skills, setSkills] = useState<Skill[]>([]);
  const [f, setF] = useState({
    title: habit?.title ?? "", kind: habit?.kind ?? "binary", targetValue: habit?.targetValue?.toString() ?? "", unit: habit?.unit ?? "",
    type: habit?.schedule.type ?? "daily", days: habit?.schedule.type === "weekdays" ? habit.schedule.days : [1, 2, 3, 4, 5],
    perWeek: habit?.schedule.type === "weekly" ? habit.schedule.perWeek : 3, goalId: habit?.goalId ?? "", skillId: habit?.skillId ?? "", reminderTime: habit?.reminderTime ?? "",
  });
  useEffect(() => {
    growth.goals.list().then(setGoals).catch(() => {});
    growth.skills.list().then(setSkills).catch(() => {});
  }, []);
  const schedule: HabitSchedule = f.type === "weekdays" ? { type: "weekdays", days: f.days } : f.type === "weekly" ? { type: "weekly", perWeek: f.perWeek } : { type: "daily" };
  const body = { title: f.title, kind: f.kind, targetValue: f.targetValue ? Number(f.targetValue) : null, unit: f.unit, schedule, goalId: f.goalId || null, skillId: f.skillId || null, reminderTime: f.reminderTime || null };
  const submit = () => (habit ? growth.habits.update(habit.id, body) : growth.habits.create(body)).then(onSaved).catch(s.fail);
  return (
    <Modal title={habit ? t("Edit habit") : t("New habit")} onClose={onClose}
      footer={<>
        {habit && <button className="btn danger" onClick={async () => { if (await s.confirm({ title: t("Delete this habit and its history?"), danger: true, confirmLabel: t("Delete") })) growth.habits.remove(habit.id).then(onSaved).catch(s.fail); }}>{t("Delete")}</button>}
        {habit && <button className="btn" onClick={() => growth.habits.update(habit.id, { archived: true }).then(onSaved).catch(s.fail)}>{t("Archive")}</button>}
        <span className="spacer" />
        <button className="btn" onClick={onClose}>{t("Cancel")}</button>
        <button className="btn primary" onClick={submit}>{t("Save")}</button>
      </>}>
      <div className="field"><label>{t("Habit")}</label><input className="input" autoFocus value={f.title} placeholder={t("e.g. Review vocabulary 20 minutes")} onChange={(e) => setF({ ...f, title: e.target.value })} /></div>
      <div className="field"><label>{t("Schedule")}</label>
        <div className="row wrap">
          {(["daily", "weekdays", "weekly"] as const).map((x) => <label key={x} className="row"><input type="radio" checked={f.type === x} onChange={() => setF({ ...f, type: x })} /> {t(x === "daily" ? "Every day" : x === "weekdays" ? "Specific weekdays" : "Times per week")}</label>)}
        </div>
        {f.type === "weekdays" && (
          <div className="row" style={{ gap: 4, marginTop: 6 }}>
            {[1, 2, 3, 4, 5, 6, 7].map((d) => (
              <button key={d} type="button" className={`btn small${f.days.includes(d) ? " primary" : ""}`} aria-pressed={f.days.includes(d)}
                onClick={() => setF({ ...f, days: f.days.includes(d) ? f.days.filter((x) => x !== d) : [...f.days, d].sort() })}>
                {new Date(Date.UTC(2024, 0, d)).toLocaleDateString(locale, { weekday: "short", timeZone: "UTC" })}
              </button>
            ))}
          </div>
        )}
        {f.type === "weekly" && <div className="row" style={{ marginTop: 6 }}><input className="input" type="number" min={1} max={7} style={{ width: 70 }} value={f.perWeek} onChange={(e) => setF({ ...f, perWeek: Number(e.target.value) })} /> {t("times per week")}</div>}
      </div>
      <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr 1fr", gap: "0 12px" }}>
        <div className="field"><label>{t("Type")}</label>
          <select className="select" value={f.kind} onChange={(e) => setF({ ...f, kind: e.target.value as Habit["kind"] })}>
            <option value="binary">{t("Yes / no")}</option><option value="count">{t("Count")}</option><option value="duration">{t("Duration")}</option><option value="numeric">{t("Number")}</option>
          </select>
        </div>
        {f.kind !== "binary" && <div className="field"><label>{t("Target")}</label><input className="input" inputMode="decimal" value={f.targetValue} onChange={(e) => setF({ ...f, targetValue: e.target.value })} /></div>}
        {f.kind !== "binary" && <div className="field"><label>{t("Unit")}</label><input className="input" value={f.unit} placeholder={t("min, pages…")} onChange={(e) => setF({ ...f, unit: e.target.value })} /></div>}
      </div>
      <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr 1fr", gap: "0 12px" }}>
        <div className="field"><label>{t("Supports goal")}</label><select className="select" value={f.goalId} onChange={(e) => setF({ ...f, goalId: e.target.value })}><option value="">—</option>{goals.map((g) => <option key={g.id} value={g.id}>{g.title}</option>)}</select></div>
        <div className="field"><label>{t("Practises skill")}</label><select className="select" value={f.skillId} onChange={(e) => setF({ ...f, skillId: e.target.value })}><option value="">—</option>{skills.map((k) => <option key={k.id} value={k.id}>{k.name}</option>)}</select></div>
        <div className="field"><label>{t("Reminder")}</label><input className="input" type="time" value={f.reminderTime} onChange={(e) => setF({ ...f, reminderTime: e.target.value })} /></div>
      </div>
    </Modal>
  );
}
