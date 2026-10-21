import { useEffect, useState } from "react";
import { useStore } from "../../app/providers/store";
import { useI18n } from "../../app/providers/i18n";
import { growth, money, pct, type ReviewSummary } from "../../services/desktop-bridge/growthApi";
import { Icon } from "../../components/ui/Icon";
import { useShield } from "../finance/FinanceView";

function shiftWeek(key: string, n: number): string {
  const [y, w] = key.split("-W").map(Number);
  // Monday of ISO week 1 is the Monday of the week containing Jan 4th.
  const jan4 = new Date(Date.UTC(y, 0, 4));
  const monday = new Date(jan4.getTime() - ((jan4.getUTCDay() + 6) % 7) * 86400000 + (w - 1 + n) * 7 * 86400000);
  const thursday = new Date(monday.getTime() + 3 * 86400000);
  const yr = thursday.getUTCFullYear();
  const firstJan4 = new Date(Date.UTC(yr, 0, 4));
  const firstMonday = new Date(firstJan4.getTime() - ((firstJan4.getUTCDay() + 6) % 7) * 86400000);
  const week = Math.round((monday.getTime() - firstMonday.getTime()) / (7 * 86400000)) + 1;
  return `${yr}-W${String(week).padStart(2, "0")}`;
}

export function ReviewView({ period: initial }: { period?: string }) {
  const s = useStore();
  const { t, locale } = useI18n();
  const [hidden] = useShield();
  const [period, setPeriod] = useState(initial ?? "");
  const [draft, setDraft] = useState<ReviewSummary | null>(null);
  const [f, setF] = useState({ reflection: "", learnings: "", nextCommitments: "" });
  const [savedAt, setSavedAt] = useState<string | null>(null);
  const [markReviewed, setMarkReviewed] = useState(true);

  useEffect(() => {
    if (!period) {
      growth.reviews.currentWeek().then((r) => setPeriod(r.period)).catch(s.fail);
      return;
    }
    growth.reviews.get(period).then((r) => {
      setDraft(r.draft);
      setF({ reflection: r.saved?.reflection ?? "", learnings: r.saved?.learnings ?? "", nextCommitments: r.saved?.nextCommitments ?? "" });
      setSavedAt(r.saved?.updatedAt ?? null);
    }).catch(s.fail);
  }, [period, s.fail]);

  const save = () => growth.reviews.save(period, { ...f, markGoalsReviewed: markReviewed }).then((r) => (setSavedAt(r.saved?.updatedAt ?? null), s.toast(t("Review saved"), { kind: "success" }))).catch(s.fail);
  const d = draft;
  const habitsAvg = d?.habits.filter((h) => h.consistency !== null);

  return (
    <div className="content">
      <div className="page">
        <div className="row" style={{ marginTop: 16 }}>
          <h1 style={{ margin: 0, fontSize: 30 }}>🗓️ {t("Weekly review")}</h1>
          <span className="spacer" />
          <button className="icon-btn" aria-label={t("Previous week")} onClick={() => setPeriod(shiftWeek(period, -1))}><Icon name="chevronLeft" /></button>
          <strong>{period}</strong>
          <button className="icon-btn" aria-label={t("Next week")} onClick={() => setPeriod(shiftWeek(period, 1))}><Icon name="chevronRight" /></button>
        </div>
        {d && <p className="muted small-text">{d.start} → {d.end} · {t("Drafted from your local data. Edit the reflection and save — nothing is sent anywhere.")}{savedAt && ` · ${t("Saved")} ${savedAt.slice(0, 16).replace("T", " ")}`}</p>}
        {!d ? <div className="loading">…</div> : (
          <>
            <div style={{ display: "grid", gridTemplateColumns: "repeat(4, 1fr)", gap: 10 }}>
              {[[t("Tasks done"), d.completedTasks.length], [t("Slipped"), d.slippedTasks.length], [t("Focus time"), `${Math.round(d.focusMinutes / 6) / 10} h`], [t("New evidence"), d.evidence.length]].map(([k, v]) => (
                <div key={k as string} className="card-box" style={{ marginBottom: 0, padding: "10px 14px" }}><div className="small-text muted">{k}</div><div style={{ fontSize: 22, fontWeight: 700 }}>{v}</div></div>
              ))}
            </div>

            <div className="h-section">{t("Goals")}</div>
            {d.goals.length === 0 ? <div className="muted small-text">{t("No active goals.")}</div> : (
              <table className="plain">
                <thead><tr><th>{t("Goal")}</th><th>{t("Health")}</th><th>{t("Preparation")}</th><th>{t("Change")}</th><th>{t("Focus vs plan")}</th></tr></thead>
                <tbody>
                  {d.goals.map((g) => (
                    <tr key={g.id}>
                      <td><button className="crumb" onClick={() => s.navigate({ name: "goal", id: g.id })}>{g.title}</button></td>
                      <td>{t(g.health)}</td>
                      <td>{pct(g.preparation)}</td>
                      <td>{g.preparationDelta === undefined ? <span className="faint">—</span> : `${g.preparationDelta >= 0 ? "+" : ""}${Math.round(g.preparationDelta * 100)} pt`}</td>
                      <td>{Math.round(g.focusMinutes / 6) / 10} h{g.plannedMinutes ? ` / ${Math.round(g.plannedMinutes / 6) / 10} h` : ""}</td>
                    </tr>
                  ))}
                </tbody>
              </table>
            )}

            <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: 16 }}>
              <div>
                <div className="h-section">✅ {t("Completed")}</div>
                {d.completedTasks.length ? d.completedTasks.map((x) => <div key={x.id} className="small-text">• {x.title}</div>) : <div className="faint small-text">—</div>}
                {d.milestonesMet.length > 0 && <><div className="h-section">◆ {t("Milestones met")}</div>{d.milestonesMet.map((x) => <div key={x.id} className="small-text">• {x.title} <span className="faint">({x.goal})</span></div>)}</>}
              </div>
              <div>
                <div className="h-section">↪︎ {t("Slipped (due this week, not done)")}</div>
                {d.slippedTasks.length ? d.slippedTasks.map((x) => <div key={x.id} className="small-text">• {x.title} <span className="faint">{x.dueDate}</span></div>) : <div className="faint small-text">—</div>}
              </div>
              <div>
                <div className="h-section">🧠 {t("Skill evidence added")}</div>
                {d.evidence.length ? d.evidence.map((x) => <div key={x.id} className="small-text">• {x.skill}: {x.title}{x.level !== null && ` (L${x.level})`}</div>) : <div className="faint small-text">{t("No new evidence — skill levels did not change this week.")}</div>}
              </div>
              <div>
                <div className="h-section">🔁 {t("Habit consistency")}</div>
                {habitsAvg?.length ? d.habits.map((h) => <div key={h.id} className="small-text">• {h.title}: {h.done}/{h.perWeek ?? h.due - h.skipped} {h.skipped > 0 && <span className="faint">({t("{n} skipped", { n: h.skipped })})</span>} — {pct(h.consistency)}</div>) : <div className="faint small-text">—</div>}
              </div>
              <div>
                <div className="h-section">💰 {t("Spending")}</div>
                {d.spending.length ? d.spending.map((x, i) => <div key={i} className="small-text">• {t(x.category)}: {money(x.totalMinor, x.currency, locale, hidden)}</div>) : <div className="faint small-text">—</div>}
              </div>
            </div>

            <div className="h-section">{t("Reflection")}</div>
            <div className="field"><label>{t("What went well / problems")}</label><textarea className="textarea" rows={3} value={f.reflection} onChange={(e) => setF({ ...f, reflection: e.target.value })} /></div>
            <div className="field"><label>{t("What I learned")}</label><textarea className="textarea" rows={3} value={f.learnings} onChange={(e) => setF({ ...f, learnings: e.target.value })} /></div>
            <div className="field"><label>{t("Next week commitments")}</label><textarea className="textarea" rows={3} value={f.nextCommitments} onChange={(e) => setF({ ...f, nextCommitments: e.target.value })} /></div>
            <div className="row">
              <label className="row small-text"><input type="checkbox" checked={markReviewed} onChange={(e) => setMarkReviewed(e.target.checked)} /> {t("Mark active goals as reviewed")}</label>
              <span className="spacer" />
              <button className="btn primary" onClick={save}>{t("Save review")}</button>
            </div>
          </>
        )}
      </div>
    </div>
  );
}
