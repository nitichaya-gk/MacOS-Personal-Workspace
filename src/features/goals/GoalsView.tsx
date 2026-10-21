import { useEffect, useState } from "react";
import { useStore } from "../../app/providers/store";
import { useI18n } from "../../app/providers/i18n";
import { growth, pct } from "../../services/desktop-bridge/growthApi";
import type { Goal } from "../../shared/types";
import { formatDate } from "../../shared/utils";
import { EmptyState, Modal } from "../../components/ui/Modal";
import { Bar, DOMAIN_ICON, DOMAINS, HealthBadge } from "./ui";

export function GoalForm({ onClose, onCreated }: { onClose: () => void; onCreated: (g: Goal) => void }) {
  const s = useStore();
  const { t } = useI18n();
  const [f, setF] = useState({ title: "", domain: "education", targetDate: "", successDefinition: "", motivation: "", timeBudgetHoursPerWeek: "" });
  const set = (k: keyof typeof f, v: string) => setF((x) => ({ ...x, [k]: v }));
  const submit = async () => {
    try {
      const g = await growth.goals.create({
        ...f, status: "active", targetDate: f.targetDate || null,
        timeBudgetHoursPerWeek: f.timeBudgetHoursPerWeek ? Number(f.timeBudgetHoursPerWeek) : null,
      });
      onCreated(g);
    } catch (e) {
      s.fail(e);
    }
  };
  return (
    <Modal title={t("New goal")} onClose={onClose} footer={<><button className="btn" onClick={onClose}>{t("Cancel")}</button><button className="btn primary" onClick={submit}>{t("Create goal")}</button></>}>
      <div className="field"><label>{t("Goal")}</label><input className="input" autoFocus value={f.title} placeholder={t("What outcome do you want?")} onChange={(e) => set("title", e.target.value)} /></div>
      <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: "0 12px" }}>
        <div className="field"><label>{t("Domain")}</label>
          <select className="select" value={f.domain} onChange={(e) => set("domain", e.target.value)}>{DOMAINS.map((d) => <option key={d} value={d}>{DOMAIN_ICON[d]} {t(d)}</option>)}</select>
        </div>
        <div className="field"><label>{t("Target date")}</label><input className="input" type="date" value={f.targetDate} onChange={(e) => set("targetDate", e.target.value)} /></div>
      </div>
      <div className="field">
        <label>{t("Success definition (measurable)")}</label>
        <textarea className="textarea" rows={2} value={f.successDefinition} placeholder={t("How will you know it is achieved?")} onChange={(e) => set("successDefinition", e.target.value)} />
      </div>
      <div className="field"><label>{t("Why it matters")}</label><input className="input" value={f.motivation} onChange={(e) => set("motivation", e.target.value)} /></div>
      <div className="field"><label>{t("Time budget (hours / week)")}</label><input className="input" inputMode="decimal" style={{ width: 120 }} value={f.timeBudgetHoursPerWeek} onChange={(e) => set("timeBudgetHoursPerWeek", e.target.value)} /></div>
    </Modal>
  );
}

export function GoalsView({ domain: initialDomain }: { domain?: string }) {
  const s = useStore();
  const { t, locale } = useI18n();
  const [domain, setDomain] = useState<string | null>(initialDomain ?? null);
  const [closed, setClosed] = useState(false);
  const [goals, setGoals] = useState<Goal[] | null>(null);
  const [creating, setCreating] = useState(false);
  const load = () => growth.goals.list(domain, closed).then(setGoals).catch(s.fail);
  useEffect(() => {
    load();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [domain, closed, s.taskDialog]);

  const example = async () => {
    const g = await growth.goals.createExample().catch((e) => (s.fail(e), null));
    if (g) s.navigate({ name: "goal", id: g.id });
  };

  return (
    <div className="content">
      <div className="page" style={{ maxWidth: 1040 }}>
        <div className="row" style={{ marginTop: 16 }}>
          <h1 style={{ margin: 0, fontSize: 30 }}>🎯 {t("Goals & Roadmaps")}</h1>
          <span className="spacer" />
          <button className="btn primary" onClick={() => setCreating(true)}>＋ {t("New goal")}</button>
        </div>
        <p className="muted small-text">{t("Track outcomes and evidence, not just checkboxes. Each goal shows task completion, roadmap progress, requirement preparation and skill readiness separately.")}</p>
        <div className="tabs" role="tablist">
          <button role="tab" className={`tab${domain === null ? " active" : ""}`} onClick={() => setDomain(null)}>{t("All")}</button>
          {DOMAINS.map((d) => <button key={d} role="tab" className={`tab${domain === d ? " active" : ""}`} onClick={() => setDomain(d)}>{DOMAIN_ICON[d]} {t(d)}</button>)}
          <span className="spacer" />
          <label className="row small-text muted" style={{ paddingRight: 6 }}><input type="checkbox" checked={closed} onChange={(e) => setClosed(e.target.checked)} /> {t("Show achieved & cancelled")}</label>
        </div>
        {goals === null ? (
          <div className="loading">…</div>
        ) : goals.length === 0 ? (
          <EmptyState icon="🎯" title={t("No goals yet")} hint={t("Start with a goal, break it into requirements, milestones and tasks, then attach evidence as you go.")}>
            <button className="btn primary" onClick={() => setCreating(true)}>＋ {t("New goal")}</button>
            <button className="btn" onClick={example}>{t("Try the CEDT CU example (illustrative)")}</button>
          </EmptyState>
        ) : (
          <div style={{ display: "grid", gridTemplateColumns: "repeat(auto-fill, minmax(300px, 1fr))", gap: 12 }}>
            {goals.map((g) => {
              const m = g.metrics!;
              return (
                <div key={g.id} className="tile" style={{ minHeight: 0, gap: 8 }} onClick={() => s.navigate({ name: "goal", id: g.id })} role="link" tabIndex={0}
                  onKeyDown={(e) => e.key === "Enter" && s.navigate({ name: "goal", id: g.id })}>
                  <div className="row">
                    <span style={{ fontSize: 20 }}>{DOMAIN_ICON[g.domain]}</span>
                    <span className="tile-title" style={{ flex: 1 }}>{g.title}</span>
                  </div>
                  <div className="row wrap" style={{ gap: 6 }}>
                    <HealthBadge health={m.health} />
                    <span className="chip">{t(g.status)}</span>
                    {g.isExample && <span className="chip c-yellow">{t("Example")}</span>}
                    {g.targetDate && <span className="small-text muted">📅 {formatDate(g.targetDate, locale)}</span>}
                  </div>
                  <div className="small-text" style={{ display: "grid", gridTemplateColumns: "auto 1fr auto", gap: "4px 8px", alignItems: "center" }}>
                    <span className="muted">{t("Preparation")}</span><Bar value={m.preparation} color="var(--success)" /><span>{pct(m.preparation)}</span>
                    <span className="muted">{t("Tasks")}</span><Bar value={m.taskCompletion} /><span>{m.tasksDone}/{m.tasksTotal}</span>
                    <span className="muted">{t("Roadmap")}</span><Bar value={m.roadmapProgress} color="var(--warning)" /><span>{pct(m.roadmapProgress)}</span>
                    <span className="muted">{t("Skills")}</span><Bar value={m.skillReadiness} color="#9a6dd7" /><span>{pct(m.skillReadiness)}</span>
                  </div>
                  {(m.unknownCount > 0 || m.unverified > 0) && (
                    <div className="small-text" style={{ color: "var(--warning)" }}>
                      {m.unverified > 0 && `◆ ${t("{n} criteria not verified", { n: m.unverified })} `}
                      {m.unknownCount > 0 && `? ${t("{n} with unknown progress", { n: m.unknownCount })}`}
                    </div>
                  )}
                </div>
              );
            })}
          </div>
        )}
        {goals && goals.length > 0 && <LooseEnds />}
      </div>
      {creating && <GoalForm onClose={() => setCreating(false)} onCreated={(g) => (setCreating(false), s.navigate({ name: "goal", id: g.id }))} />}
    </div>
  );
}

type Orphans = Awaited<ReturnType<typeof growth.orphans>>;

/** Rules-based check for work that is not connected to anything (PRD: orphan detection). */
function LooseEnds() {
  const s = useStore();
  const { t } = useI18n();
  const [data, setData] = useState<Orphans | null>(null);
  const [open, setOpen] = useState<string | null>(null);
  useEffect(() => {
    growth.orphans().then(setData).catch(s.fail);
  }, [s.fail, s.taskDialog]);
  if (!data) return null;
  const rows: { key: keyof Orphans; label: string; hint: string; go: (id: string) => void }[] = [
    { key: "goalsWithoutSuccessDefinition", label: "Active goals without a success definition", hint: "Write how you will know it is achieved.", go: (id) => s.navigate({ name: "goal", id }) },
    { key: "goalsWithoutRequirements", label: "Goals without requirements", hint: "Break the goal into the criteria it needs.", go: (id) => s.navigate({ name: "goal", id, tab: "requirements" }) },
    { key: "requirementsWithoutWork", label: "Unmet requirements with no task or milestone", hint: "Nothing is planned for these yet.", go: (id) => s.navigate({ name: "goal", id, tab: "roadmap" }) },
    { key: "tasksWithoutGoalOrProject", label: "Open tasks not linked to a goal or project", hint: "Fine for chores — link the ones that serve a goal.", go: (id) => s.openTask(id) },
    { key: "skillsWithoutEvidence", label: "Skills without any evidence", hint: "Add a test, review or project to assess the level.", go: (id) => s.navigate({ name: "skills", id }) },
  ];
  const total = rows.reduce((n, r) => n + data[r.key].length, 0);
  return (
    <section className="card-box" style={{ marginTop: 24 }}>
      <h3>🧩 {t("Loose ends")} · {total}</h3>
      <p className="desc">{total ? t("Items that are not connected to the rest of your plan. Nothing is wrong with them — this is a prompt to link or tidy.") : t("Everything is connected. Nice.")}</p>
      {rows.filter((r) => data[r.key].length).map((r) => (
        <div key={r.key} style={{ borderTop: "1px solid var(--border)", padding: "6px 0" }}>
          <button className="row small-text" style={{ width: "100%", background: "none", border: 0, padding: 0, color: "inherit", textAlign: "left" }} aria-expanded={open === r.key} onClick={() => setOpen(open === r.key ? null : r.key)}>
            <span style={{ width: 14 }}>{open === r.key ? "▾" : "▸"}</span>
            <span style={{ flex: 1 }}>{t(r.label)}</span>
            <strong>{data[r.key].length}</strong>
          </button>
          {open === r.key && (
            <div style={{ paddingLeft: 20, marginTop: 4 }}>
              <div className="small-text faint" style={{ marginBottom: 4 }}>{t(r.hint)}</div>
              {data[r.key].map((x, i) => <div key={i}><button className="crumb small-text" onClick={() => r.go(x.id)}>{x.title || t("Untitled")}</button></div>)}
            </div>
          )}
        </div>
      ))}
    </section>
  );
}
