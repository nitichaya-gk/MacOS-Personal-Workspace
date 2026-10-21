import { useEffect, useState } from "react";
import { useStore } from "../../app/providers/store";
import { useI18n } from "../../app/providers/i18n";
import { growth, money, pct } from "../../services/desktop-bridge/growthApi";
import type { FocusSession, Goal, Habit, MonthSummary, Task } from "../../shared/types";
import { HabitRow } from "../habits/HabitsView";
import { useShield } from "../finance/FinanceView";
import { Bar, HealthBadge } from "./ui";
import { TodaySchedule, type Block } from "../timetable/TimetableView";

/** Start/stop focus timer. The running session is stored in the database, so it survives restarts. */
export function FocusTimer({ tasks, onStopped }: { tasks: Task[]; onStopped?: () => void }) {
  const s = useStore();
  const { t } = useI18n();
  const [running, setRunning] = useState<FocusSession | null>(null);
  const [taskId, setTaskId] = useState("");
  const [now, setNow] = useState(Date.now());
  const [today, setToday] = useState(0);
  const load = () => growth.today().then((p) => (setRunning(p.runningSession), setToday(p.focusMinutesToday))).catch(() => {});
  useEffect(() => {
    load();
    const timer = setInterval(() => setNow(Date.now()), 1000);
    return () => clearInterval(timer);
  }, []);
  const elapsed = running ? Math.max(0, Math.floor((now - Date.parse(running.startedAt)) / 1000)) : 0;
  const mmss = `${String(Math.floor(elapsed / 60)).padStart(2, "0")}:${String(elapsed % 60).padStart(2, "0")}`;
  return (
    <div className="card-box" style={{ padding: "10px 14px" }}>
      <div className="row wrap">
        <strong>⏱ {t("Focus")}</strong>
        {running ? (
          <>
            <span style={{ fontVariantNumeric: "tabular-nums", fontSize: 20, fontWeight: 700 }} aria-live="off">{mmss}</span>
            <span className="small-text muted">{running.taskTitle ?? running.goalTitle ?? t("Free focus")}</span>
            <span className="spacer" />
            <button className="btn small ghost" onClick={() => growth.focus.discard(running.id).then(load).catch(s.fail)}>{t("Discard")}</button>
            <button className="btn small primary" onClick={async () => {
              const note = await s.prompt({ title: t("Session notes (optional)"), placeholder: t("What did you work on?") });
              if (note === null) return;
              await growth.focus.stop(running.id, { note }).catch(s.fail);
              s.toast(t("Logged {m} min", { m: Math.round(elapsed / 60) }), { kind: "success" });
              load();
              onStopped?.();
            }}>■ {t("Stop & log")}</button>
          </>
        ) : (
          <>
            <select className="select" style={{ flex: 1, minWidth: 160, height: 28 }} value={taskId} onChange={(e) => setTaskId(e.target.value)} aria-label={t("Focus on task")}>
              <option value="">{t("No specific task")}</option>
              {tasks.filter((x) => x.status !== "done").map((x) => <option key={x.id} value={x.id}>{x.title}</option>)}
            </select>
            <button className="btn small primary" onClick={() => growth.focus.start({ taskId: taskId || undefined, plannedMinutes: 45 }).then(load).catch(s.fail)}>▶ {t("Start")}</button>
          </>
        )}
        <span className="small-text faint">{t("{m} min today", { m: today })}</span>
      </div>
    </div>
  );
}

export function GoalPulse({ goals }: { goals: Goal[] }) {
  const s = useStore();
  const { t } = useI18n();
  if (!goals.length)
    return <div className="muted small-text">{t("No active goals.")} <button className="crumb" onClick={() => s.navigate({ name: "goals" })}>{t("Create one")} →</button></div>;
  return (
    <div style={{ display: "grid", gridTemplateColumns: "repeat(auto-fill, minmax(230px, 1fr))", gap: 10 }}>
      {goals.map((g) => (
        <div key={g.id} className="tile" style={{ minHeight: 0 }} onClick={() => s.navigate({ name: "goal", id: g.id })} role="link" tabIndex={0}>
          <span className="tile-title">{g.title}</span>
          <div className="row"><HealthBadge health={g.metrics!.health} /><span className="spacer" /><span className="small-text muted">{t("prep")} {pct(g.metrics!.preparation)}</span></div>
          <Bar value={g.metrics!.preparation} color="var(--success)" />
        </div>
      ))}
    </div>
  );
}

export function TodayGrowth() {
  const s = useStore();
  const { t, locale } = useI18n();
  const [plan, setPlan] = useState<{ goals: Goal[]; habits: Habit[]; tasks: Task[]; schedule?: unknown[] } | null>(null);
  const [month, setMonth] = useState<MonthSummary | null>(null);
  const [hidden, toggle] = useShield();
  const [budgetOpen, setBudgetOpen] = useState(false);
  const load = () => growth.today().then(setPlan).catch(() => {});
  useEffect(() => {
    load();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [s.taskDialog, s.counts]);
  useEffect(() => {
    if (budgetOpen && s.counts) growth.finance.month(s.counts.date.slice(0, 7)).then(setMonth).catch(() => {});
  }, [budgetOpen, s.counts]);
  if (!plan) return null;
  return (
    <>
      <div className="h-section"><span>📅 {t("Today's schedule")}</span><span className="spacer" /><button className="btn small ghost" onClick={() => s.navigate({ name: "timetable" })}>{t("Timetable")} →</button></div>
      <TodaySchedule blocks={(plan.schedule ?? []) as Block[]} onChanged={load} />
      <div className="h-section"><span>🎯 {t("Goal pulse")}</span><span className="spacer" /><button className="btn small ghost" onClick={() => s.navigate({ name: "goals" })}>{t("All goals")} →</button></div>
      <GoalPulse goals={plan.goals} />
      <div style={{ marginTop: 12 }}><FocusTimer tasks={plan.tasks} onStopped={load} /></div>
      {plan.habits.length > 0 && (
        <>
          <div className="h-section"><span>🔁 {t("Habits due today")}</span><span className="spacer" /><button className="btn small ghost" onClick={() => s.navigate({ name: "habits" })}>{t("All habits")} →</button></div>
          {plan.habits.map((h) => <HabitRow key={h.id} h={h} compact onChanged={load} />)}
        </>
      )}
      <div className="h-section">
        <button className="crumb" style={{ fontWeight: 600 }} onClick={() => setBudgetOpen(!budgetOpen)} aria-expanded={budgetOpen}>{budgetOpen ? "▾" : "▸"} 💰 {t("Budget pulse")}</button>
        <span className="spacer" />
        {budgetOpen && <button className="btn small ghost" onClick={toggle}>{hidden ? "👁" : "🙈"}</button>}
      </div>
      {budgetOpen && (month?.currencies.length ? month.currencies.map((c) => (
        <div key={c.currency} className="small-text" style={{ display: "grid", gridTemplateColumns: "repeat(3, 1fr)", gap: 8 }}>
          <div>{t("Spent this month")}: <strong>{money(c.expenseMinor, c.currency, locale, hidden)}</strong></div>
          <div>{t("Net flow")}: <strong>{money(c.netFlowMinor, c.currency, locale, hidden)}</strong></div>
          <div>{c.budgets.filter((b) => b.over).length > 0 ? <span style={{ color: "var(--danger)" }}>⚠︎ {t("{n} budget(s) over", { n: c.budgets.filter((b) => b.over).length })}</span> : t("Budgets OK")}</div>
        </div>
      )) : <div className="muted small-text">{t("No finance data yet.")}</div>)}
    </>
  );
}
