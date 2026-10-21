import { useCallback, useEffect, useState } from "react";
import { useStore } from "../../app/providers/store";
import { useI18n } from "../../app/providers/i18n";
import { growth } from "../../services/desktop-bridge/growthApi";
import type { Evidence, Skill } from "../../shared/types";
import { Icon } from "../../components/ui/Icon";
import { EmptyState } from "../../components/ui/Modal";
import { Bar, EvidenceDialog, levelLabel } from "../goals/ui";

export function SkillsView({ selected }: { selected?: string }) {
  const s = useStore();
  const { t } = useI18n();
  const [skills, setSkills] = useState<Skill[] | null>(null);
  const [sel, setSel] = useState<string | undefined>(selected);
  const [adding, setAdding] = useState<string | null>(null);
  const load = useCallback(() => growth.skills.list().then((x) => (setSkills(x), setSel((cur) => cur ?? x[0]?.id))).catch(s.fail), [s.fail]);
  useEffect(() => {
    load();
  }, [load]);
  const create = async () => {
    const name = await s.prompt({ title: t("New skill"), placeholder: t("e.g. Algorithms, English reading, React") });
    if (!name?.trim()) return;
    const k = await growth.skills.create({ name }).catch((e) => (s.fail(e), null));
    if (k) (await load(), setSel(k.id));
  };
  const current = skills?.find((x) => x.id === sel);
  const byCategory = new Map<string, Skill[]>();
  for (const k of skills ?? []) {
    const c = k.category || t("Uncategorised");
    if (!byCategory.has(c)) byCategory.set(c, []);
    byCategory.get(c)!.push(k);
  }

  return (
    <div className="content">
      <div className="page" style={{ maxWidth: 1100 }}>
        <div className="row" style={{ marginTop: 16 }}>
          <h1 style={{ margin: 0, fontSize: 30 }}>🧠 {t("Learning & skills")}</h1>
          <span className="spacer" />
          <button className="btn" onClick={() => setAdding(sel ?? "")}>＋ {t("Evidence")}</button>
          <button className="btn primary" onClick={create}>＋ {t("New skill")}</button>
        </div>
        <p className="muted small-text">{t("A skill level changes only when you add evidence scored against that skill's rubric (a test, a review, a project). Hours and completed tasks are shown as practice, not as growth.")}</p>
        {skills === null ? <div className="loading">…</div> : skills.length === 0 ? (
          <EmptyState icon="🧠" title={t("No skills yet")} hint={t("Add the skills your goals need, define what each level means, then attach evidence.")}>
            <button className="btn primary" onClick={create}>＋ {t("New skill")}</button>
          </EmptyState>
        ) : (
          <div style={{ display: "grid", gridTemplateColumns: "320px 1fr", gap: 20 }}>
            <div>
              {[...byCategory.entries()].map(([cat, list]) => (
                <div key={cat}>
                  <div className="menu-label" style={{ paddingLeft: 0 }}>{cat}</div>
                  {list.map((k) => (
                    <button key={k.id} className={`nav-item${sel === k.id ? " active" : ""}`} style={{ height: "auto", padding: "6px 8px", flexDirection: "column", alignItems: "stretch" }} onClick={() => setSel(k.id)}>
                      <div className="row"><span className="label">{k.name}</span><span className="small-text muted">{k.readiness.currentLevel === null ? "—" : `L${k.readiness.currentLevel}`} / L{k.targetLevel}</span></div>
                      <Bar value={k.readiness.currentLevel === null ? null : k.readiness.currentLevel / Math.max(1, k.targetLevel)} color="#9a6dd7" height={4} />
                    </button>
                  ))}
                </div>
              ))}
            </div>
            {current && <SkillDetail key={current.id} skill={current} reload={load} onEvidence={() => setAdding(current.id)} />}
          </div>
        )}
      </div>
      {adding !== null && <EvidenceDialog preset={{ skillId: adding || undefined }} onClose={() => setAdding(null)} onSaved={load} />}
    </div>
  );
}

function SkillDetail({ skill, reload, onEvidence }: { skill: Skill; reload: () => void; onEvidence: () => void }) {
  const s = useStore();
  const { t } = useI18n();
  const [evidence, setEvidence] = useState<Evidence[]>([]);
  const [rubric, setRubric] = useState<string[]>(skill.rubric.length === 6 ? skill.rubric : ["", "", "", "", "", ""]);
  const [editRubric, setEditRubric] = useState(false);
  const loadEv = useCallback(() => growth.skills.evidence(skill.id).then(setEvidence).catch(s.fail), [skill.id, s.fail]);
  useEffect(() => {
    loadEv();
  }, [loadEv, skill]);
  const upd = (patch: Record<string, unknown>) => growth.skills.update(skill.id, patch).then(reload).catch(s.fail);
  const r = skill.readiness;
  return (
    <div>
      <div className="row">
        <input className="title-input" style={{ fontSize: 26, margin: 0 }} defaultValue={skill.name} onBlur={(e) => e.target.value.trim() && e.target.value !== skill.name && upd({ name: e.target.value })} aria-label={t("Skill name")} />
        <button className="icon-btn" aria-label={t("Delete skill")} onClick={async () => { if (await s.confirm({ title: t("Delete this skill?"), message: t("Its evidence is kept in backups but hidden from views."), danger: true, confirmLabel: t("Delete") })) growth.skills.remove(skill.id).then(reload).catch(s.fail); }}><Icon name="trash" /></button>
      </div>
      <div className="row wrap" style={{ margin: "6px 0 12px" }}>
        <label className="row small-text muted">{t("Category")} <input className="input" style={{ width: 160, height: 26 }} defaultValue={skill.category} onBlur={(e) => e.target.value !== skill.category && upd({ category: e.target.value })} /></label>
        <label className="row small-text muted">{t("Target level")}
          <select className="select" style={{ width: 80, height: 26 }} value={skill.targetLevel} onChange={(e) => upd({ targetLevel: Number(e.target.value) })}>{[1, 2, 3, 4, 5].map((l) => <option key={l} value={l}>L{l}</option>)}</select>
        </label>
      </div>
      <div style={{ display: "grid", gridTemplateColumns: "repeat(4, 1fr)", gap: 10 }}>
        <div className="card-box" style={{ marginBottom: 0, padding: "10px 12px" }}><div className="small-text muted">{t("Assessed level")}</div><div style={{ fontSize: 22, fontWeight: 700 }}>{r.currentLevel === null ? "—" : `L${r.currentLevel}`}</div><div className="small-text faint">{levelLabel(skill.rubric, r.currentLevel).replace(/^L\d · /, "")}</div></div>
        <div className="card-box" style={{ marginBottom: 0, padding: "10px 12px" }}><div className="small-text muted">{t("Confidence")}</div><div style={{ fontSize: 18, fontWeight: 600 }}>{t(r.confidence)}</div><div className="small-text faint">{t("{n} recent evidence", { n: r.recentEvidenceCount })}</div></div>
        <div className="card-box" style={{ marginBottom: 0, padding: "10px 12px" }}><div className="small-text muted">{t("Last assessed")}</div><div style={{ fontSize: 16, fontWeight: 600 }}>{r.lastAssessedAt ?? "—"}</div>{r.stale && <span className="chip c-yellow">{t("stale — reassess")}</span>}</div>
        <div className="card-box" style={{ marginBottom: 0, padding: "10px 12px" }}><div className="small-text muted">{t("Practice (done tasks)")}</div><div style={{ fontSize: 22, fontWeight: 700 }}>{skill.practiceTasks}</div><div className="small-text faint">{t("not counted as level")}</div></div>
      </div>
      {skill.goals.length > 0 && <div className="small-text muted" style={{ marginTop: 10 }}>🎯 {t("Needed by")}: {skill.goals.map((g) => <button key={g.id} className="crumb" onClick={() => s.navigate({ name: "goal", id: g.id, tab: "skills" })}>{g.title} (L{g.targetLevel})</button>)}</div>}

      <div className="h-section"><span>{t("Rubric")} · v{skill.rubricVersion}</span><span className="spacer" /><button className="btn small ghost" onClick={() => setEditRubric(!editRubric)}>{editRubric ? t("Cancel") : t("Edit rubric")}</button></div>
      {rubric.map((desc, i) => (
        <div key={i} className="row small-text" style={{ marginBottom: 4 }}>
          <span className="chip" style={{ width: 34, justifyContent: "center" }}>L{i}</span>
          {editRubric ? <input className="input" value={desc} onChange={(e) => setRubric(rubric.map((x, j) => (j === i ? e.target.value : x)))} /> : <span>{desc || <span className="faint">—</span>}</span>}
        </div>
      ))}
      {editRubric && <button className="btn small primary" onClick={() => upd({ rubric }).then(() => setEditRubric(false))}>{t("Save rubric (new version)")}</button>}

      <div className="h-section"><span>{t("Evidence")}</span><span className="spacer" /><button className="btn small" onClick={onEvidence}>＋ {t("Add evidence")}</button></div>
      {evidence.length === 0 ? <div className="muted small-text">{t("No evidence yet. Add a test result, a reviewed piece of work or a project.")}</div> : evidence.map((e) => (
        <div key={e.id} className="task-row" style={{ cursor: "default" }}>
          <span style={{ width: 40 }} className="chip">{e.level === null ? "—" : `L${e.level}`}</span>
          <div className="t">
            <div className="name">{e.title}</div>
            <div className="task-meta">
              <span>{t(e.evidenceType)}</span><span>📅 {e.observedAt}</span>
              {e.score !== null && <span>{e.score}{e.maxScore !== null ? ` / ${e.maxScore}` : ""}</span>}
              {e.rubricVersion !== null && <span>{t("rubric")} v{e.rubricVersion}</span>}
              {e.taskTitle && <span>✅ {e.taskTitle}</span>}
              {e.reviewer && <span>👤 {e.reviewer}</span>}
              {e.sourceUrl && <a href="#" onClick={(ev) => (ev.preventDefault(), import("../../services/desktop-bridge/api").then((m) => m.api.system.openExternal(e.sourceUrl)))}>🔗 {t("link")}</a>}
            </div>
            {e.notes && <div className="small-text muted">{e.notes}</div>}
          </div>
          <button className="icon-btn small" aria-label={t("Delete evidence")} onClick={() => growth.skills.deleteEvidence(e.id).then(() => (loadEv(), reload())).catch(s.fail)}><Icon name="trash" size={13} /></button>
        </div>
      ))}
    </div>
  );
}
