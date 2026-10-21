import { useEffect, useState, type ReactNode } from "react";
import { useStore } from "../../app/providers/store";
import { useI18n } from "../../app/providers/i18n";
import { growth, pct } from "../../services/desktop-bridge/growthApi";
import { api } from "../../services/desktop-bridge/api";
import type { Health, Skill, Task } from "../../shared/types";
import { Modal } from "../../components/ui/Modal";
import { Popover, useAnchor } from "../../components/ui/Popover";

export const DOMAIN_ICON: Record<string, string> = { education: "🎓", career: "💼", finance: "💰", personal: "🌱", project: "🛠️" };
export const DOMAINS = ["education", "career", "finance", "personal", "project"] as const;

const HEALTH: Record<string, { icon: string; label: string; color: string }> = {
  on_track: { icon: "●", label: "On track", color: "var(--success)" },
  at_risk: { icon: "▲", label: "At risk", color: "var(--warning)" },
  blocked: { icon: "■", label: "Blocked", color: "var(--danger)" },
  needs_review: { icon: "◆", label: "Needs review", color: "var(--accent)" },
  inactive: { icon: "○", label: "Inactive", color: "var(--text-faint)" },
  achieved: { icon: "★", label: "Achieved", color: "var(--success)" },
};

/** Health uses a distinct shape per state so it is not conveyed by colour alone. */
export function HealthBadge({ health, withReasons }: { health: Health; withReasons?: boolean }) {
  const { t } = useI18n();
  const h = HEALTH[health.state] ?? HEALTH.inactive;
  const pop = useAnchor();
  return (
    <>
      <button className="chip" style={{ color: h.color, background: "var(--bg-hover)", border: 0 }} onClick={withReasons ? pop.toggle : undefined} title={health.reasons.join("\n")}>
        {h.icon} {t(h.label)}{health.reasons.length > 0 && withReasons ? ` · ${health.reasons.length}` : ""}
      </button>
      {pop.anchor && (
        <Popover anchor={pop.anchor} onClose={pop.close} width={340}>
          <div style={{ padding: 8 }}>
            <div className="menu-label" style={{ paddingLeft: 0 }}>{t("Why")}</div>
            {health.reasons.length ? health.reasons.map((r, i) => <div key={i} className="small-text" style={{ marginBottom: 4 }}>• {t(r)}</div>) : <div className="small-text muted">{t("No issues found by the rules.")}</div>}
            <div className="small-text faint" style={{ marginTop: 8 }}>{t("Health is rules-based (deadlines, progress pace, blockers, verification, reviews). It is not a prediction of success.")}</div>
          </div>
        </Popover>
      )}
    </>
  );
}

export function Bar({ value, color, height = 6, label }: { value: number | null | undefined; color?: string; height?: number; label?: string }) {
  const v = value === null || value === undefined ? null : Math.max(0, Math.min(1, value));
  return (
    <div role="progressbar" aria-label={label} aria-valuemin={0} aria-valuemax={100} aria-valuenow={v === null ? undefined : Math.round(v * 100)}
      style={{ height, borderRadius: height, background: "var(--bg-active)", overflow: "hidden", width: "100%" }}>
      {v !== null && <div style={{ width: `${v * 100}%`, height: "100%", background: color ?? "var(--accent)", borderRadius: height }} />}
    </div>
  );
}

/** One progress dimension with its definition, limitation and formula one click away. */
export function MetricCard({ title, value, detail, formula, note, color }: { title: string; value: number | null; detail: ReactNode; formula: string; note?: string; color?: string }) {
  const { t } = useI18n();
  const pop = useAnchor();
  return (
    <div className="card-box" style={{ marginBottom: 0, padding: "12px 14px" }}>
      <div className="row">
        <span className="small-text muted" style={{ fontWeight: 600 }}>{title}</span>
        <span className="spacer" />
        <button className="icon-btn small" aria-label={t("How is this calculated?")} onClick={pop.toggle}>ⓘ</button>
      </div>
      <div style={{ fontSize: 26, fontWeight: 700, margin: "2px 0 6px" }}>{value === null ? <span className="faint">—</span> : pct(value)}</div>
      <Bar value={value} color={color} label={title} />
      <div className="small-text muted" style={{ marginTop: 6 }}>{detail}</div>
      {pop.anchor && (
        <Popover anchor={pop.anchor} onClose={pop.close} width={340}>
          <div style={{ padding: 8 }} className="small-text">
            <div style={{ fontWeight: 600, marginBottom: 4 }}>{title}</div>
            <div className="mono" style={{ whiteSpace: "normal", marginBottom: 6 }}>{t(formula)}</div>
            {note && <div className="muted">{t(note)}</div>}
          </div>
        </Popover>
      )}
    </div>
  );
}

export const levelLabel = (rubric: string[] | undefined, level: number | null | undefined) =>
  level === null || level === undefined ? "—" : `L${level}${rubric?.[level] ? ` · ${rubric[level]}` : ""}`;

/** Add evidence (assessment, mock test, review, artifact…) to a skill, optionally tied to a task/requirement. */
export function EvidenceDialog({ preset, onClose, onSaved }: { preset?: { skillId?: string; requirementId?: string; taskId?: string; title?: string }; onClose: () => void; onSaved?: () => void }) {
  const s = useStore();
  const { t } = useI18n();
  const [skills, setSkills] = useState<Skill[]>([]);
  const [tasks, setTasks] = useState<Task[]>([]);
  const [f, setF] = useState({
    skillId: preset?.skillId ?? "", newSkill: "", evidenceType: "assessment", title: preset?.title ?? "", level: "", score: "", maxScore: "",
    observedAt: s.counts?.date ?? "", taskId: preset?.taskId ?? "", sourceUrl: "", reviewer: "", notes: "", updateRequirementValue: !!preset?.requirementId,
  });
  useEffect(() => {
    growth.skills.list().then((x) => (setSkills(x), !f.skillId && x[0] && setF((v) => ({ ...v, skillId: x[0].id })))).catch(s.fail);
    api.tasks.list("all", { limit: 300 }).then(setTasks).catch(() => {});
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);
  const skill = skills.find((x) => x.id === f.skillId);
  const set = (k: keyof typeof f, v: string | boolean) => setF((x) => ({ ...x, [k]: v }));
  const num = (v: string) => (v.trim() === "" ? null : Number(v));
  const submit = async () => {
    try {
      let skillId = f.skillId;
      if (f.skillId === "__new") {
        if (!f.newSkill.trim()) return s.toast(t("Name the new skill."), { kind: "error" });
        skillId = (await growth.skills.create({ name: f.newSkill })).id;
      }
      await growth.skills.addEvidence({
        skillId, evidenceType: f.evidenceType, title: f.title, level: num(f.level), score: num(f.score), maxScore: num(f.maxScore),
        observedAt: f.observedAt || undefined, taskId: f.taskId || undefined, requirementId: preset?.requirementId, sourceUrl: f.sourceUrl,
        reviewer: f.reviewer, notes: f.notes, updateRequirementValue: f.updateRequirementValue,
      });
      s.toast(t("Evidence added"), { kind: "success" });
      onSaved?.();
      onClose();
    } catch (e) {
      s.fail(e);
    }
  };
  return (
    <Modal title={t("Add evidence")} onClose={onClose} wide footer={<><button className="btn" onClick={onClose}>{t("Cancel")}</button><button className="btn primary" onClick={submit}>{t("Add evidence")}</button></>}>
      <p className="small-text muted" style={{ marginTop: 0 }}>{t("Evidence is what changes a skill level — a score, a reviewed piece of work, a test result. Finishing tasks alone does not.")}</p>
      <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: "0 14px" }}>
        <div className="field">
          <label>{t("Skill")}</label>
          <select className="select" value={f.skillId} onChange={(e) => set("skillId", e.target.value)}>
            {skills.map((x) => <option key={x.id} value={x.id}>{x.name}</option>)}
            <option value="__new">＋ {t("New skill…")}</option>
          </select>
          {f.skillId === "__new" && <input className="input" style={{ marginTop: 4 }} placeholder={t("Skill name")} value={f.newSkill} onChange={(e) => set("newSkill", e.target.value)} />}
        </div>
        <div className="field">
          <label>{t("Type")}</label>
          <select className="select" value={f.evidenceType} onChange={(e) => set("evidenceType", e.target.value)}>
            {["assessment", "mock_test", "code_review", "project", "artifact", "certificate", "self_review"].map((x) => <option key={x} value={x}>{t(x)}</option>)}
          </select>
        </div>
        <div className="field" style={{ gridColumn: "1 / 3" }}>
          <label>{t("What is the evidence?")}</label>
          <input className="input" autoFocus value={f.title} placeholder={t("e.g. Mock test set A, PR #12 reviewed")} onChange={(e) => set("title", e.target.value)} />
        </div>
        <div className="field">
          <label>{t("Rubric level (optional)")}</label>
          <select className="select" value={f.level} onChange={(e) => set("level", e.target.value)}>
            <option value="">{t("Not assessed against the rubric")}</option>
            {[0, 1, 2, 3, 4, 5].map((l) => <option key={l} value={l}>{levelLabel(skill?.rubric, l)}</option>)}
          </select>
        </div>
        <div className="field">
          <label>{t("Score (optional)")}</label>
          <div className="row"><input className="input" inputMode="decimal" value={f.score} onChange={(e) => set("score", e.target.value)} /> / <input className="input" inputMode="decimal" placeholder="100" value={f.maxScore} onChange={(e) => set("maxScore", e.target.value)} /></div>
        </div>
        <div className="field">
          <label>{t("Date observed")}</label>
          <input className="input" type="date" value={f.observedAt} onChange={(e) => set("observedAt", e.target.value)} />
        </div>
        <div className="field">
          <label>{t("From task (optional)")}</label>
          <select className="select" value={f.taskId} onChange={(e) => set("taskId", e.target.value)}>
            <option value="">—</option>
            {tasks.map((x) => <option key={x.id} value={x.id}>{x.title}</option>)}
          </select>
        </div>
        <div className="field"><label>{t("Link (optional)")}</label><input className="input" placeholder="https://…" value={f.sourceUrl} onChange={(e) => set("sourceUrl", e.target.value)} /></div>
        <div className="field"><label>{t("Reviewer (optional)")}</label><input className="input" value={f.reviewer} onChange={(e) => set("reviewer", e.target.value)} /></div>
      </div>
      <div className="field"><label>{t("Notes")}</label><textarea className="textarea" rows={2} value={f.notes} onChange={(e) => set("notes", e.target.value)} /></div>
      {preset?.requirementId && (
        <label className="row small-text"><input type="checkbox" checked={f.updateRequirementValue} onChange={(e) => set("updateRequirementValue", e.target.checked)} /> {t("Use the score as this requirement's measured value")}</label>
      )}
    </Modal>
  );
}
