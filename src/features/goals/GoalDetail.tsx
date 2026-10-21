import { useCallback, useEffect, useMemo, useState } from "react";
import { useStore } from "../../app/providers/store";
import { useI18n } from "../../app/providers/i18n";
import { api, errorMessage } from "../../services/desktop-bridge/api";
import { growth, money, pct, type GoalDetail as Detail } from "../../services/desktop-bridge/growthApi";
import type { Goal, Milestone, Skill } from "../../shared/types";
import { formatDate } from "../../shared/utils";
import { Icon } from "../../components/ui/Icon";
import { Gantt, type GanttItem, type GanttScale } from "../../components/ui/Gantt";
import { EmptyState } from "../../components/ui/Modal";
import { TaskRow } from "../tasks/TaskRow";
import { useShield } from "../finance/FinanceView";
import { Bar, DOMAIN_ICON, DOMAINS, EvidenceDialog, HealthBadge, MetricCard, levelLabel } from "./ui";

type Tab = "overview" | "requirements" | "roadmap" | "timeline" | "tasks" | "skills" | "finance" | "drafts";

const SAT = ["not_started", "in_progress", "met", "not_met", "blocked", "unknown"] as const;
const SAT_ICON: Record<string, string> = { not_started: "○", in_progress: "◐", met: "●", not_met: "✕", blocked: "■", unknown: "?" };
const REQ_TYPES = ["binary", "numeric", "score", "document", "portfolio", "external_verification"] as const;

export function GoalDetailView({ id, initialTab }: { id: string; initialTab?: string }) {
  const s = useStore();
  const { t, locale } = useI18n();
  const [d, setD] = useState<Detail | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [tab, setTab] = useState<Tab>((initialTab as Tab) ?? "overview");
  const [evidenceFor, setEvidenceFor] = useState<{ requirementId?: string; skillId?: string } | null>(null);
  const load = useCallback(() => growth.goals.detail(id).then((x) => (setD(x), setError(null))).catch((e) => setError(errorMessage(e))), [id]);
  useEffect(() => {
    load();
  }, [load, s.taskDialog]);

  if (error) return <div className="content"><div className="error-box">{t("This goal could not be opened.")} <span className="mono">{error}</span></div></div>;
  if (!d) return <div className="content"><div className="loading">{t("Loading…")}</div></div>;
  const g = d.goal;
  const m = d.metrics;
  const save = (patch: Partial<Goal> & Record<string, unknown>) => growth.goals.update(g.id, patch).then(load).catch(s.fail);

  const tabs: { id: Tab; label: string; count?: number }[] = [
    { id: "overview", label: t("Overview") },
    { id: "requirements", label: t("Requirements"), count: d.requirements.length },
    { id: "roadmap", label: t("Roadmap"), count: d.milestones.length },
    { id: "timeline", label: t("Timeline") },
    { id: "tasks", label: t("Tasks"), count: d.tasks.length },
    { id: "skills", label: t("Skills & evidence"), count: m.skillReadiness.total },
    { id: "finance", label: t("Finance") },
    ...(d.drafts.length ? [{ id: "drafts" as Tab, label: t("AI drafts"), count: d.drafts.length }] : []),
  ];

  return (
    <div className="content">
      <div className="page" style={{ maxWidth: 1040 }}>
        {g.isExample && (
          <div className="banner" style={{ marginTop: 12 }}>
            ⚠️ {t("Illustrative scenario: these criteria are NOT the real admission requirements. Check the official announcement for your round and year, then replace them and record the source and date.")}
          </div>
        )}
        <div className="row" style={{ marginTop: 16, alignItems: "flex-start" }}>
          <span style={{ fontSize: 34 }}>{DOMAIN_ICON[g.domain]}</span>
          <input className="title-input" style={{ fontSize: 30, margin: 0 }} defaultValue={g.title} key={g.version}
            onBlur={(e) => e.target.value.trim() && e.target.value !== g.title && save({ title: e.target.value })} aria-label={t("Goal")} />
        </div>
        <div className="row wrap" style={{ margin: "6px 0 14px" }}>
          <HealthBadge health={m.health} withReasons />
          <select className="select" style={{ width: 130, height: 26 }} value={g.status} aria-label={t("Status")} onChange={(e) => save({ status: e.target.value as Goal["status"] })}>
            {["draft", "active", "paused", "achieved", "cancelled"].map((x) => <option key={x} value={x}>{t(x)}</option>)}
          </select>
          <select className="select" style={{ width: 140, height: 26 }} value={g.domain} aria-label={t("Domain")} onChange={(e) => save({ domain: e.target.value as Goal["domain"] })}>
            {DOMAINS.map((x) => <option key={x} value={x}>{DOMAIN_ICON[x]} {t(x)}</option>)}
          </select>
          <label className="row small-text muted">{t("Target")} <input className="input" type="date" style={{ width: 150, height: 26 }} defaultValue={g.targetDate ?? ""} key={`td${g.version}`} onBlur={(e) => e.target.value !== (g.targetDate ?? "") && save({ targetDate: e.target.value || null })} /></label>
          <select className="select" style={{ width: 120, height: 26 }} value={g.priority} aria-label={t("Priority")} onChange={(e) => save({ priority: Number(e.target.value) })}>
            <option value={1}>{t("High")}</option><option value={2}>{t("Medium")}</option><option value={3}>{t("Low")}</option>
          </select>
          <span className="spacer" />
          <button className="btn small" onClick={() => save({ markReviewed: true }).then(() => s.toast(t("Marked as reviewed")))}>✓ {t("Mark reviewed")}</button>
          <button className="btn small danger" onClick={async () => {
            if (await s.confirm({ title: t("Move this goal to the Trash?"), message: t("Tasks stay; their links to this goal's requirements are kept until you delete it forever."), danger: true, confirmLabel: t("Move to Trash") })) {
              await growth.goals.remove(g.id).catch(s.fail);
              s.navigate({ name: "goals" }, { replace: true });
            }
          }}><Icon name="trash" size={13} /></button>
        </div>
        <div className="tabs" role="tablist">
          {tabs.map((x) => (
            <button key={x.id} role="tab" aria-selected={tab === x.id} className={`tab${tab === x.id ? " active" : ""}`} onClick={() => setTab(x.id)}>
              {x.label} {!!x.count && <span className="count">{x.count}</span>}
            </button>
          ))}
        </div>

        {tab === "overview" && <Overview d={d} save={save} setTab={setTab} />}
        {tab === "requirements" && <Requirements d={d} reload={load} onEvidence={(rid) => setEvidenceFor({ requirementId: rid })} />}
        {tab === "roadmap" && <Roadmap d={d} reload={load} />}
        {tab === "timeline" && <GoalTimeline d={d} reload={load} />}
        {tab === "tasks" && (
          <div>
            {d.tasks.length === 0 ? <EmptyState icon="✅" title={t("No tasks linked yet")} hint={t("Add tasks to milestones in the Roadmap tab, or link existing tasks to requirements from the task dialog.")} /> :
              d.tasks.map((x) => (
                <div key={x.id}>
                  <TaskRow task={x} onChanged={load} />
                  {x.requirementLinks.filter((l) => l.goalId === g.id).length > 0 && (
                    <div className="small-text faint" style={{ padding: "0 0 6px 36px" }}>
                      → {x.requirementLinks.filter((l) => l.goalId === g.id).map((l) => `${l.requirementTitle} (${l.allocationPct}%)`).join(" · ")}
                      {x.skills.length > 0 && ` · ${t("practises")} ${x.skills.map((k) => k.name).join(", ")}`}
                    </div>
                  )}
                </div>
              ))}
          </div>
        )}
        {tab === "skills" && <Skills d={d} reload={load} onEvidence={(sid) => setEvidenceFor({ skillId: sid })} />}
        {tab === "finance" && <GoalFinance d={d} locale={locale} />}
        {tab === "drafts" && <Drafts d={d} reload={load} />}
      </div>
      {evidenceFor && <EvidenceDialog preset={evidenceFor} onClose={() => setEvidenceFor(null)} onSaved={load} />}
    </div>
  );
}

function Overview({ d, save, setTab }: { d: Detail; save: (p: Record<string, unknown>) => Promise<unknown>; setTab: (t: Tab) => void }) {
  const { t } = useI18n();
  const m = d.metrics;
  const g = d.goal;
  const next = d.tasks.filter((x) => x.status !== "done").slice(0, 5);
  return (
    <>
      <div style={{ display: "grid", gridTemplateColumns: "repeat(auto-fit, minmax(210px, 1fr))", gap: 10 }}>
        <MetricCard title={t("Task completion")} value={m.taskCompletion.value} formula={m.taskCompletion.formula} note={m.taskCompletion.note}
          detail={t("{done} of {total} tasks done", { done: m.taskCompletion.done, total: m.taskCompletion.total })} />
        <MetricCard title={t("Roadmap progress")} value={m.roadmapProgress.value} formula={m.roadmapProgress.formula} color="var(--warning)"
          detail={t("{met} of {n} milestones met", { met: m.roadmapProgress.met, n: m.roadmapProgress.milestones })} />
        <MetricCard title={t("Requirement preparation")} value={m.preparation.value} formula={m.preparation.formula} color="var(--success)"
          note="Preparation progress only — it does not say the goal will be achieved."
          detail={<>{t("{met} of {total} requirements met", { met: m.preparation.met, total: m.preparation.total })}{m.preparation.unknownCount > 0 && <> · <span style={{ color: "var(--warning)" }}>? {t("{n} unknown", { n: m.preparation.unknownCount })}</span></>}</>} />
        <MetricCard title={t("Skill readiness (estimate)")} value={m.skillReadiness.value} formula={m.skillReadiness.formula} color="#9a6dd7"
          detail={t("{a} of {n} skills assessed", { a: m.skillReadiness.assessed, n: m.skillReadiness.total })} />
      </div>
      <div className="card-box" style={{ marginTop: 12 }}>
        <strong>{t("Where this goal stands")}</strong>
        <p className="muted small-text" style={{ margin: "4px 0 8px" }}>
          {t("{tasks} of the roadmap's task effort is done; {unverified} criteria are not verified and {unknown} have unknown progress.", { tasks: pct(m.taskCompletion.value), unverified: m.preparation.unverified, unknown: m.preparation.unknownCount })}
        </p>
        {m.achievementBlockers.length > 0 && g.status !== "achieved" && (
          <div className="small-text">
            <div className="muted" style={{ fontWeight: 600 }}>{t("Before it can be marked achieved:")}</div>
            {m.achievementBlockers.map((b, i) => <div key={i}>• {b}</div>)}
          </div>
        )}
        {m.preparation.unverified > 0 && <button className="btn small" style={{ marginTop: 8 }} onClick={() => setTab("requirements")}>◆ {t("Verify criteria")}</button>}
      </div>
      <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: 12 }}>
        <div className="field">
          <label>{t("Success definition (measurable)")}</label>
          <textarea className="textarea" rows={3} defaultValue={g.successDefinition} key={`sd${g.version}`} onBlur={(e) => e.target.value !== g.successDefinition && save({ successDefinition: e.target.value })} />
        </div>
        <div className="field">
          <label>{t("Why it matters")}</label>
          <textarea className="textarea" rows={3} defaultValue={g.motivation} key={`mo${g.version}`} onBlur={(e) => e.target.value !== g.motivation && save({ motivation: e.target.value })} />
        </div>
        <div className="field">
          <label>{t("Time budget (hours / week)")}</label>
          <input className="input" style={{ width: 120 }} inputMode="decimal" defaultValue={g.timeBudgetHoursPerWeek ?? ""} key={`tb${g.version}`}
            onBlur={(e) => save({ timeBudgetHoursPerWeek: e.target.value ? Number(e.target.value) : null })} />
        </div>
        <div className="field">
          <label>{t("Last reviewed")}</label>
          <span>{g.lastReviewedAt ?? t("Never")}</span>
        </div>
      </div>
      <div className="h-section">{t("Next actions")}</div>
      {next.length ? next.map((x) => <TaskRow key={x.id} task={x} />) : <div className="muted small-text">{t("No open tasks. Add milestones and tasks in the Roadmap tab.")}</div>}
    </>
  );
}

function Requirements({ d, reload, onEvidence }: { d: Detail; reload: () => void; onEvidence: (requirementId: string) => void }) {
  const s = useStore();
  const { t, locale } = useI18n();
  const [open, setOpen] = useState<string | null>(null);
  const [title, setTitle] = useState("");
  const progress = useMemo(() => new Map(d.metrics.preparation.requirements.map((r) => [r.id, r])), [d]);
  const upd = (id: string, patch: Record<string, unknown>) => growth.goals.updateRequirement(id, patch).then(reload).catch(s.fail);
  const add = () => title.trim() && growth.goals.addRequirement(d.goal.id, { title }).then((r) => (setTitle(""), setOpen(r.id), reload())).catch(s.fail);

  return (
    <div>
      {d.requirements.map((r) => {
        const p = progress.get(r.id);
        const vstate = p?.verificationState ?? "not_needed";
        return (
          <div key={r.id} className="card-box" style={{ padding: "10px 14px", marginBottom: 8 }}>
            <div className="row" style={{ cursor: "default" }} onClick={() => setOpen(open === r.id ? null : r.id)}>
              <span title={t(r.satisfactionStatus)} aria-label={t(r.satisfactionStatus)} style={{ width: 18 }}>{SAT_ICON[r.satisfactionStatus]}</span>
              <strong style={{ flex: 1 }}>{r.title}</strong>
              {r.critical && <span className="chip c-red">{t("Critical")}</span>}
              {(vstate === "unverified" || vstate === "outdated") && <span className="chip c-yellow">◆ {t(vstate)}</span>}
              {vstate === "verified" && <span className="chip c-green">✓ {t("verified")}</span>}
              <span className="small-text muted" style={{ width: 54, textAlign: "right" }}>{p?.progress === null || p === undefined ? "?" : pct(p.progress)}</span>
              <div style={{ width: 90 }}><Bar value={p?.progress} color="var(--success)" /></div>
              <Icon name={open === r.id ? "chevronDown" : "chevronRight"} size={14} />
            </div>
            <div className="small-text faint" style={{ paddingLeft: 26 }}>
              {t(r.type)} · {t("weight")} {r.weight} · {p?.explanation ? t(p.explanation) : ""} · {t("{n} tasks", { n: r.taskCount })} · {t("{n} evidence", { n: r.evidence.length })}
              {r.deadline && ` · 📅 ${formatDate(r.deadline, locale)}`}
            </div>
            {open === r.id && (
              <div style={{ marginTop: 10, paddingLeft: 26 }}>
                <div style={{ display: "grid", gridTemplateColumns: "repeat(3, 1fr)", gap: "0 12px" }}>
                  <div className="field" style={{ gridColumn: "1 / 4" }}><label>{t("Title")}</label><input className="input" defaultValue={r.title} onBlur={(e) => e.target.value !== r.title && upd(r.id, { title: e.target.value })} /></div>
                  <div className="field"><label>{t("Type")}</label>
                    <select className="select" value={r.type} onChange={(e) => upd(r.id, { type: e.target.value })}>{REQ_TYPES.map((x) => <option key={x} value={x}>{t(x)}</option>)}</select>
                  </div>
                  <div className="field"><label>{t("Status")}</label>
                    <select className="select" value={r.satisfactionStatus} onChange={(e) => upd(r.id, { satisfactionStatus: e.target.value })}>{SAT.map((x) => <option key={x} value={x}>{SAT_ICON[x]} {t(x)}</option>)}</select>
                  </div>
                  <div className="field"><label>{t("Weight")}</label><input className="input" inputMode="decimal" defaultValue={r.weight} onBlur={(e) => Number(e.target.value) !== r.weight && upd(r.id, { weight: Number(e.target.value) })} /></div>
                  {(r.type === "numeric" || r.type === "score") && (
                    <>
                      <div className="field"><label>{t("Target value")}</label><input className="input" inputMode="decimal" defaultValue={r.targetValue ?? ""} onBlur={(e) => upd(r.id, { targetValue: e.target.value === "" ? null : Number(e.target.value) })} /></div>
                      <div className="field"><label>{t("Current (measured) value")}</label><input className="input" inputMode="decimal" defaultValue={r.currentValue ?? ""} onBlur={(e) => upd(r.id, { currentValue: e.target.value === "" ? null : Number(e.target.value) })} /></div>
                      <div className="field"><label>{t("Unit")}</label><input className="input" defaultValue={r.unit} onBlur={(e) => upd(r.id, { unit: e.target.value })} /></div>
                    </>
                  )}
                  <div className="field"><label>{t("Deadline")}</label><input className="input" type="date" defaultValue={r.deadline ?? ""} onBlur={(e) => upd(r.id, { deadline: e.target.value || null })} /></div>
                  <label className="row small-text" style={{ alignSelf: "center" }}><input type="checkbox" checked={r.critical} onChange={(e) => upd(r.id, { critical: e.target.checked })} /> {t("Critical — must be met for the goal")}</label>
                </div>
                <div style={{ display: "grid", gridTemplateColumns: "2fr 1fr 1fr", gap: "0 12px" }}>
                  <div className="field"><label>{t("Official source (URL or document)")}</label><input className="input" defaultValue={r.verificationSource} placeholder="https://…" onBlur={(e) => e.target.value !== r.verificationSource && upd(r.id, { verificationSource: e.target.value })} /></div>
                  <div className="field"><label>{t("Verification")}</label>
                    <select className="select" value={r.verificationStatus} onChange={(e) => upd(r.id, { verificationStatus: e.target.value })}>
                      <option value="unverified">{t("unverified")}</option><option value="verified">{t("verified")}</option><option value="outdated">{t("outdated")}</option>
                    </select>
                  </div>
                  <div className="field"><label>{t("Checked on")}</label><input className="input" type="date" defaultValue={r.sourceCheckedAt ?? ""} onBlur={(e) => upd(r.id, { sourceCheckedAt: e.target.value || null })} /></div>
                </div>
                <div className="field"><label>{t("Description / criteria text")}</label><textarea className="textarea" rows={2} defaultValue={r.description} onBlur={(e) => e.target.value !== r.description && upd(r.id, { description: e.target.value })} /></div>
                {(r.type === "numeric" || r.type === "score") && (
                  <div className="field"><label>{t("Override reason (needed to mark met below target)")}</label><input className="input" defaultValue={r.manualOverrideReason} onBlur={(e) => e.target.value !== r.manualOverrideReason && upd(r.id, { manualOverrideReason: e.target.value })} /></div>
                )}
                <div className="row" style={{ margin: "4px 0" }}>
                  <strong className="small-text">{t("Evidence")}</strong>
                  <span className="spacer" />
                  <button className="btn small" onClick={() => onEvidence(r.id)}>＋ {t("Add evidence")}</button>
                  <button className="btn small danger" onClick={async () => { if (await s.confirm({ title: t("Delete this requirement?"), message: t("Task links to it are removed; tasks stay."), danger: true, confirmLabel: t("Delete") })) growth.goals.deleteRequirement(r.id).then(reload).catch(s.fail); }}>{t("Delete")}</button>
                </div>
                {r.evidence.length === 0 ? <div className="small-text faint">{t("No evidence yet.")}</div> : r.evidence.map((e) => (
                  <div key={e.id} className="small-text" style={{ padding: "3px 0" }}>
                    📎 {e.title} · {e.skillName} {e.level !== null && `· L${e.level}`} {e.score !== null && `· ${e.score}${e.maxScore ? `/${e.maxScore}` : ""}`} · <span className="faint">{e.observedAt}</span>
                  </div>
                ))}
              </div>
            )}
          </div>
        );
      })}
      <div className="row" style={{ marginTop: 8 }}>
        <input className="input" placeholder={t("New requirement, e.g. English score at the official threshold")} value={title} onChange={(e) => setTitle(e.target.value)} onKeyDown={(e) => e.key === "Enter" && !e.nativeEvent.isComposing && add()} />
        <button className="btn primary" onClick={add}>＋ {t("Add")}</button>
      </div>
    </div>
  );
}

function Roadmap({ d, reload }: { d: Detail; reload: () => void }) {
  const s = useStore();
  const { t, locale } = useI18n();
  const [form, setForm] = useState({ title: "", phase: "", requirementId: "", dueDate: "" });
  const [taskTitle, setTaskTitle] = useState<Record<string, string>>({});
  const phases = useMemo(() => {
    const map = new Map<string, Milestone[]>();
    for (const m of d.milestones) {
      const k = m.phase || t("No phase");
      if (!map.has(k)) map.set(k, []);
      map.get(k)!.push(m);
    }
    return [...map.entries()];
  }, [d, t]);
  const reqTitle = (id: string | null) => d.requirements.find((r) => r.id === id)?.title;
  const addMs = () =>
    form.title.trim() &&
    growth.goals.addMilestone(d.goal.id, { ...form, requirementId: form.requirementId || null, dueDate: form.dueDate || null }).then(() => (setForm({ ...form, title: "", dueDate: "" }), reload())).catch(s.fail);
  const addTask = (mid: string) => {
    const title = taskTitle[mid]?.trim();
    if (!title) return;
    growth.goals.addMilestoneTask(mid, { title, projectId: d.goal.projectId ?? undefined }).then(() => (setTaskTitle({ ...taskTitle, [mid]: "" }), reload(), s.refreshCounts())).catch(s.fail);
  };
  const unplanned = d.tasks.filter((x) => !x.milestoneId);

  return (
    <div>
      <p className="small-text muted">{t("Phase → milestone → task. Tasks you add here appear in Today and Tasks — they are the same records, not copies.")}</p>
      {phases.map(([phase, list]) => (
        <div key={phase} style={{ marginBottom: 18 }}>
          <div className="h-section" style={{ marginTop: 8 }}>{phase}</div>
          {list.map((m) => {
            const tasks = d.tasks.filter((x) => x.milestoneId === m.id);
            const done = tasks.filter((x) => x.status === "done").length;
            return (
              <div key={m.id} className="card-box" style={{ padding: "10px 14px", marginBottom: 8 }}>
                <div className="row wrap">
                  <strong style={{ flex: 1 }}>{m.status === "met" ? "● " : m.status === "in_progress" ? "◐ " : m.status === "missed" ? "✕ " : "○ "}{m.title}</strong>
                  {reqTitle(m.requirementId) && <span className="chip">→ {reqTitle(m.requirementId)}</span>}
                  {m.dueDate && <span className={`small-text ${m.status !== "met" && m.dueDate < (s.counts?.date ?? "") ? "" : "muted"}`} style={m.status !== "met" && m.dueDate < (s.counts?.date ?? "") ? { color: "var(--danger)" } : undefined}>📅 {formatDate(m.dueDate, locale)}</span>}
                  <span className="small-text muted">{done}/{tasks.length}</span>
                  <select className="select" style={{ width: 130, height: 24, fontSize: 12 }} value={m.status} aria-label={t("Milestone status")} onChange={(e) => growth.goals.updateMilestone(m.id, { status: e.target.value }).then(reload).catch(s.fail)}>
                    {["planned", "in_progress", "met", "missed"].map((x) => <option key={x} value={x}>{t(x)}</option>)}
                  </select>
                  <button className="icon-btn small" aria-label={t("Delete milestone")} onClick={async () => { if (await s.confirm({ title: t("Delete this milestone?"), message: t("Its tasks stay and lose the milestone link."), danger: true, confirmLabel: t("Delete") })) growth.goals.deleteMilestone(m.id).then(reload).catch(s.fail); }}><Icon name="trash" size={13} /></button>
                </div>
                {m.acceptanceCriteria && <div className="small-text muted">{t("Acceptance")}: {m.acceptanceCriteria}</div>}
                <div style={{ marginTop: 6 }}>
                  {tasks.map((x) => (
                    <div key={x.id}>
                      <TaskRow task={x} compact onChanged={reload} />
                      {x.dependsOn.length > 0 && <div className="small-text faint" style={{ paddingLeft: 30 }}>⛓ {t("after")} {x.dependsOn.map((dep) => `${dep.status === "done" ? "✓" : "…"} ${dep.title}`).join(", ")}</div>}
                    </div>
                  ))}
                  <input className="input" style={{ marginTop: 4, height: 28 }} placeholder={t("Add a task to this milestone and press Enter")} value={taskTitle[m.id] ?? ""}
                    onChange={(e) => setTaskTitle({ ...taskTitle, [m.id]: e.target.value })} onKeyDown={(e) => e.key === "Enter" && !e.nativeEvent.isComposing && addTask(m.id)} />
                </div>
              </div>
            );
          })}
        </div>
      ))}
      {!d.milestones.length && <EmptyState icon="🗺️" title={t("No milestones yet")} hint={t("Add milestones per requirement, then tasks under each milestone.")} />}
      <div className="card-box">
        <strong className="small-text">{t("New milestone")}</strong>
        <div style={{ display: "grid", gridTemplateColumns: "2fr 1fr 1.5fr 1fr auto", gap: 8, marginTop: 8 }}>
          <input className="input" placeholder={t("Milestone")} value={form.title} onChange={(e) => setForm({ ...form, title: e.target.value })} onKeyDown={(e) => e.key === "Enter" && addMs()} />
          <input className="input" placeholder={t("Phase")} list="phases" value={form.phase} onChange={(e) => setForm({ ...form, phase: e.target.value })} />
          <datalist id="phases">{phases.map(([p]) => <option key={p} value={p} />)}</datalist>
          <select className="select" value={form.requirementId} onChange={(e) => setForm({ ...form, requirementId: e.target.value })} aria-label={t("Requirement")}>
            <option value="">{t("No requirement")}</option>
            {d.requirements.map((r) => <option key={r.id} value={r.id}>{r.title}</option>)}
          </select>
          <input className="input" type="date" value={form.dueDate} onChange={(e) => setForm({ ...form, dueDate: e.target.value })} aria-label={t("Due date")} />
          <button className="btn primary" onClick={addMs}>＋</button>
        </div>
      </div>
      {unplanned.length > 0 && (
        <>
          <div className="h-section">{t("Linked tasks without a milestone")}</div>
          {unplanned.map((x) => <TaskRow key={x.id} task={x} onChanged={reload} />)}
        </>
      )}
    </div>
  );
}

function Skills({ d, reload, onEvidence }: { d: Detail; reload: () => void; onEvidence: (skillId: string) => void }) {
  const s = useStore();
  const { t } = useI18n();
  const [all, setAll] = useState<Skill[]>([]);
  const [pick, setPick] = useState("");
  useEffect(() => {
    growth.skills.list().then(setAll).catch(() => {});
  }, [d]);
  const rubricOf = (id: string) => all.find((x) => x.id === id)?.rubric;
  const linked = d.metrics.skillReadiness.skills;
  const add = async () => {
    if (!pick) return;
    let id = pick;
    if (pick === "__new") {
      const name = await s.prompt({ title: t("New skill"), placeholder: t("e.g. Algorithms, English reading") });
      if (!name?.trim()) return;
      id = (await growth.skills.create({ name })).id;
    }
    await growth.goals.setSkill(d.goal.id, id, 3).then(reload).catch(s.fail);
    setPick("");
  };
  return (
    <div>
      <p className="small-text muted">{t("Levels come only from evidence scored against each skill's rubric. Practising a skill in a task is tracked, but does not raise the level by itself.")}</p>
      {linked.length > 0 && (
        <table className="plain">
          <thead><tr><th>{t("Skill")}</th><th>{t("Target")}</th><th>{t("Assessed")}</th><th>{t("Gap")}</th><th>{t("Confidence")}</th><th>{t("Last assessed")}</th><th /></tr></thead>
          <tbody>
            {linked.map((k) => (
              <tr key={k.skillId}>
                <td><button className="crumb" onClick={() => s.navigate({ name: "skills", id: k.skillId })}>{k.name}</button></td>
                <td>
                  <select className="select" style={{ width: 70, height: 24 }} value={k.targetLevel} onChange={(e) => growth.goals.setSkill(d.goal.id, k.skillId, Number(e.target.value)).then(reload).catch(s.fail)} aria-label={t("Target level")}>
                    {[1, 2, 3, 4, 5].map((l) => <option key={l} value={l}>L{l}</option>)}
                  </select>
                </td>
                <td title={levelLabel(rubricOf(k.skillId), k.currentLevel)}>{k.currentLevel === null ? <span className="faint">{t("not assessed")}</span> : `L${k.currentLevel}`}</td>
                <td>{k.gap === null ? "—" : k.gap === 0 ? "✓" : `−${k.gap}`}</td>
                <td>{t(k.confidence)}{k.stale && <span className="chip c-yellow" style={{ marginLeft: 4 }}>{t("stale")}</span>}</td>
                <td className="muted">{k.lastAssessedAt ?? "—"}</td>
                <td style={{ whiteSpace: "nowrap" }}>
                  <button className="btn small" onClick={() => onEvidence(k.skillId)}>＋ {t("Evidence")}</button>{" "}
                  <button className="icon-btn small" aria-label={t("Remove")} onClick={() => growth.goals.setSkill(d.goal.id, k.skillId, null).then(reload).catch(s.fail)}><Icon name="close" size={12} /></button>
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      )}
      <div className="row" style={{ marginTop: 12 }}>
        <select className="select" style={{ width: 280 }} value={pick} onChange={(e) => setPick(e.target.value)} aria-label={t("Add a skill")}>
          <option value="">{t("Add a required skill…")}</option>
          {all.filter((x) => !linked.some((l) => l.skillId === x.id)).map((x) => <option key={x.id} value={x.id}>{x.name}</option>)}
          <option value="__new">＋ {t("New skill…")}</option>
        </select>
        <button className="btn" onClick={add} disabled={!pick}>{t("Add")}</button>
      </div>
    </div>
  );
}

function GoalFinance({ d, locale }: { d: Detail; locale: string }) {
  const { t } = useI18n();
  const s = useStore();
  const [hidden, toggle] = useShield();
  const f = d.finance;
  return (
    <div>
      <div className="row" style={{ marginBottom: 8 }}>
        <span className="small-text muted">{t("Expenses and savings linked to this goal. Amounts are private by default.")}</span>
        <span className="spacer" />
        <button className="btn small" onClick={toggle}>{hidden ? `👁 ${t("Show amounts")}` : `🙈 ${t("Hide amounts")}`}</button>
      </div>
      <div className="row wrap" style={{ gap: 10 }}>
        {f.spent.length === 0 && <div className="muted">{t("No expenses linked yet. Link a transaction to this goal in Finance.")}</div>}
        {f.spent.map((x) => <div key={x.currency} className="card-box" style={{ marginBottom: 0 }}><div className="small-text muted">{t("Spent on this goal")}</div><div style={{ fontSize: 22, fontWeight: 700 }}>{money(x.spentMinor, x.currency, locale, hidden)}</div></div>)}
      </div>
      {f.savingsGoals.length > 0 && <div className="h-section">{t("Savings goals")}</div>}
      {f.savingsGoals.map((x) => (
        <div key={x.id} className="row" style={{ marginBottom: 6 }}>
          <span style={{ width: 200 }}>{x.title}</span>
          <div style={{ flex: 1 }}><Bar value={x.progress} color="var(--success)" /></div>
          <span className="small-text">{money(x.savedMinor, x.currency, locale, hidden)} / {money(x.targetMinor, x.currency, locale, hidden)}</span>
        </div>
      ))}
      {f.transactions.length > 0 && <div className="h-section">{t("Linked transactions")}</div>}
      {f.transactions.map((x) => (
        <div key={x.id} className="list-row"><span className="t">{x.merchantOrSource || x.categoryName || t(x.type)}</span><span className="small-text muted">{x.occurredOn}</span><span>{money(x.amountMinor, x.currency, locale, hidden)}</span></div>
      ))}
      <button className="btn small" style={{ marginTop: 10 }} onClick={() => s.navigate({ name: "finance" })}>💰 {t("Open Finance")}</button>
    </div>
  );
}

function Drafts({ d, reload }: { d: Detail; reload: () => void }) {
  const s = useStore();
  const { t } = useI18n();
  return (
    <div>
      <p className="small-text muted">{t("Proposals from AI clients via MCP. Review them; nothing is created until you apply.")}</p>
      {d.drafts.map((dr) => (
        <div key={dr.id} className="card-box">
          <div className="row">
            <strong>🤖 {t("Breakdown proposal")}</strong>
            <span className="small-text faint">{dr.source} · {dr.createdAt.slice(0, 16).replace("T", " ")}</span>
            <span className="spacer" />
            <button className="btn small" onClick={() => growth.goals.resolveDraft(dr.id, false).then(reload).catch(s.fail)}>{t("Discard")}</button>
            <button className="btn small primary" onClick={() => growth.goals.resolveDraft(dr.id, true).then((r) => (s.toast(t("Created {r} requirements, {m} milestones, {t} tasks", { r: r.created.requirements, m: r.created.milestones, t: r.created.tasks }), { kind: "success" }), reload(), s.refreshCounts())).catch(s.fail)}>{t("Apply")}</button>
          </div>
          {dr.payload.rationale && <p className="small-text muted">{dr.payload.rationale}</p>}
          {dr.payload.requirements.map((r, i) => (
            <div key={i} className="small-text" style={{ marginTop: 6 }}>
              <div>■ {r.title} {r.type && <span className="faint">({t(r.type)})</span>}</div>
              {(r.milestones ?? []).map((m, j) => (
                <div key={j} style={{ paddingLeft: 16 }}>
                  ◇ {m.phase ? `${m.phase}: ` : ""}{m.title}
                  {(m.tasks ?? []).map((x, k) => <div key={k} style={{ paddingLeft: 16 }} className="muted">☐ {x.title}{x.estimatedMinutes ? ` · ${x.estimatedMinutes} min` : ""}</div>)}
                </div>
              ))}
            </div>
          ))}
        </div>
      ))}
    </div>
  );
}

/** Milestones (diamonds) and tasks on one timeline; drag to change a due date. */
function GoalTimeline({ d, reload }: { d: Detail; reload: () => void }) {
  const s = useStore();
  const { t } = useI18n();
  const [scale, setScale] = useState<GanttScale>("month");
  const phaseOf = new Map(d.milestones.map((m) => [m.id, m.phase || t("Roadmap")]));
  const items: GanttItem[] = [
    ...(d.goal.targetDate ? [{ id: "goal-target", title: t("Target date"), start: d.goal.targetDate, end: d.goal.targetDate, milestone: true, icon: "🏁", group: t("Goal") }] : []),
    ...d.milestones.map((m) => ({ id: `m:${m.id}`, title: m.title, start: m.dueDate, end: m.dueDate, milestone: true, done: m.status === "met", icon: m.status === "missed" ? "✕" : "◆", group: phaseOf.get(m.id) })),
    ...d.tasks.map((x) => ({
      id: `t:${x.id}`, title: x.title, start: x.dueDate, end: x.dueDate, done: x.status === "done",
      group: x.milestoneId ? phaseOf.get(x.milestoneId) : t("Other tasks"),
      dependsOn: x.dependsOn.map((dep) => `t:${dep.id}`),
    })),
  ];
  // Gantt prints a group header whenever the group changes, so keep each phase together.
  const order = [t("Goal"), ...new Set(d.milestones.map((m) => phaseOf.get(m.id)!)), t("Other tasks")];
  items.sort((a, b) => order.indexOf(a.group!) - order.indexOf(b.group!) || Number(!a.milestone) - Number(!b.milestone) || (a.start ?? "9").localeCompare(b.start ?? "9"));
  const change = (id: string, _start: string, end: string) => {
    if (id === "goal-target") return growth.goals.update(d.goal.id, { targetDate: end }).then(reload).catch(s.fail);
    const [kind, raw] = id.split(":");
    (kind === "m" ? growth.goals.updateMilestone(raw, { dueDate: end }) : api.tasks.update(raw, { dueDate: end })).then(reload).catch(s.fail);
  };
  return (
    <div>
      <div className="row small-text" style={{ marginBottom: 8 }}>
        <span className="muted" style={{ flex: 1 }}>{t("Drag a milestone or task to change its due date. Arrows show task dependencies.")}</span>
        <div className="tabs" style={{ margin: 0, border: 0 }}>
          {(["week", "month", "quarter"] as const).map((k) => <button key={k} className={`tab${scale === k ? " active" : ""}`} onClick={() => setScale(k)}>{t(k[0].toUpperCase() + k.slice(1))}</button>)}
        </div>
      </div>
      <Gantt
        items={items}
        today={s.counts?.date ?? new Date().toISOString().slice(0, 10)}
        scale={scale}
        onChange={change}
        onOpen={(id) => id.startsWith("t:") && s.openTask(id.slice(2))}
        emptyHint={t("Give milestones and tasks a due date to see them here.")}
      />
    </div>
  );
}
