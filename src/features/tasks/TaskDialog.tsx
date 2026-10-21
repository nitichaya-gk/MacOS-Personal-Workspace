import { useEffect, useState } from "react";
import { isPermissionGranted, requestPermission } from "@tauri-apps/plugin-notification";
import { useStore } from "../../app/providers/store";
import { useI18n } from "../../app/providers/i18n";
import { api, isTauri } from "../../services/desktop-bridge/api";
import type { Task, TaskPriority, TaskStatus } from "../../shared/types";
import { addDays } from "../../shared/utils";
import { Modal } from "../../components/ui/Modal";
import { kindIcon } from "../../components/ui/Icon";
import { growth, type GoalOption } from "../../services/desktop-bridge/growthApi";
import type { Skill } from "../../shared/types";

/** Asks macOS for notification permission the first time a reminder is set. Today keeps working without it. */
async function ensureNotificationPermission(): Promise<boolean> {
  if (!isTauri()) return false;
  try {
    if (await isPermissionGranted()) return true;
    return (await requestPermission()) === "granted";
  } catch {
    return false;
  }
}

export function TaskDialog({ id, defaults, onClose }: { id?: string; defaults?: Record<string, unknown>; onClose: () => void }) {
  const s = useStore();
  const { t } = useI18n();
  const [task, setTask] = useState<Task | null>(null);
  const [title, setTitle] = useState((defaults?.title as string) ?? "");
  const [description, setDescription] = useState("");
  const [status, setStatus] = useState<TaskStatus>("todo");
  const [priority, setPriority] = useState<TaskPriority>("none");
  const [dueDate, setDueDate] = useState((defaults?.dueDate as string) ?? "");
  const [remind, setRemind] = useState("");
  const [projectId, setProjectId] = useState((defaults?.projectId as string) ?? "");
  const [links, setLinks] = useState<string[]>([]);
  const [linkQ, setLinkQ] = useState("");
  const [milestoneId, setMilestoneId] = useState((defaults?.milestoneId as string) ?? "");
  const [reqLinks, setReqLinks] = useState<{ requirementId: string; allocationPct: number }[]>([]);
  const [skillIds, setSkillIds] = useState<string[]>([]);
  const [estimated, setEstimated] = useState("");
  const [actual, setActual] = useState("");
  const [dependsOn, setDependsOn] = useState<string[]>([]);
  const [goalOptions, setGoalOptions] = useState<GoalOption[]>([]);
  const [skillOptions, setSkillOptions] = useState<Skill[]>([]);
  const [openTasks, setOpenTasks] = useState<Task[]>([]);
  const [showGrowth, setShowGrowth] = useState(false);
  useEffect(() => {
    growth.goals.options().then(setGoalOptions).catch(() => {});
    growth.skills.list().then(setSkillOptions).catch(() => {});
    api.tasks.list("open", { limit: 300 }).then(setOpenTasks).catch(() => {});
  }, []);
  const today = s.counts?.date ?? new Date().toISOString().slice(0, 10);
  const projects = s.tree.filter((p) => p.kind === "project" && !p.isTemplate);

  useEffect(() => {
    if (!id) return;
    api.tasks.get(id).then((x) => {
      setTask(x);
      setTitle(x.title);
      setDescription(x.description);
      setStatus(x.status);
      setPriority(x.priority);
      setDueDate(x.dueDate ?? "");
      setRemind(x.remindAtLocal ?? "");
      setProjectId(x.projectId ?? "");
      setLinks(x.links.map((l) => l.id));
      setMilestoneId(x.milestoneId ?? "");
      setReqLinks(x.requirementLinks.map((l) => ({ requirementId: l.requirementId, allocationPct: l.allocationPct })));
      setSkillIds(x.skills.map((k) => k.id));
      setEstimated(x.estimatedEffortMinutes?.toString() ?? "");
      setActual(x.actualEffortMinutes?.toString() ?? "");
      setDependsOn(x.dependsOn.map((d) => d.id));
      setShowGrowth(!!(x.milestoneId || x.requirementLinks.length || x.skills.length || x.dependsOn.length || x.estimatedEffortMinutes));
    }).catch((e) => (s.fail(e), onClose()));
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [id]);

  const submit = async () => {
    if (!title.trim()) return s.toast(t("Give the task a name."), { kind: "error" });
    if (remind && remind !== task?.remindAtLocal) {
      const granted = await ensureNotificationPermission();
      if (!granted) s.toast(t("Notifications are off for this app. The task still appears in Today; enable notifications in System Settings to get reminders."));
    }
    const patch = {
      title, description, status, priority,
      dueDate: dueDate || null,
      remindAtLocal: remind || null,
      projectId: projectId || null,
      links,
      milestoneId: milestoneId || null,
      requirementLinks: reqLinks,
      skillIds,
      dependsOn,
      estimatedEffortMinutes: estimated ? Math.round(Number(estimated)) : null,
      actualEffortMinutes: actual ? Math.round(Number(actual)) : null,
    };
    try {
      if (task) await api.tasks.update(task.id, patch);
      else await api.tasks.create({ ...patch, sourcePageId: (defaults?.sourcePageId as string) ?? undefined } as never);
      await s.refreshCounts();
      onClose();
    } catch (e) {
      s.fail(e);
    }
  };

  const remove = async () => {
    if (!task) return;
    await api.tasks.remove(task.id).catch(s.fail);
    s.refreshCounts();
    onClose();
    s.toast(t("Task moved to Trash"), { action: { label: t("Undo"), run: () => api.tasks.restore(task.id).then(() => s.refreshCounts()).catch(s.fail) } });
  };

  const quick = (label: string, date: string) => <button type="button" className="btn small" onClick={() => setDueDate(date)}>{label}</button>;
  const linkCandidates = linkQ.trim() ? s.tree.filter((p) => !links.includes(p.id) && p.title.toLowerCase().includes(linkQ.trim().toLowerCase())).slice(0, 6) : [];

  return (
    <Modal
      title={task ? t("Task") : t("New task")}
      onClose={onClose}
      wide
      footer={
        <>
          {task && <button className="btn danger" onClick={remove}>{t("Delete")}</button>}
          <span className="spacer" />
          <button className="btn" onClick={onClose}>{t("Cancel")}</button>
          <button className="btn primary" onClick={submit}>{task ? t("Save") : t("Create task")}</button>
        </>
      }
    >
      <form onSubmit={(e) => (e.preventDefault(), submit())}>
        <input className="input" style={{ fontSize: 17, height: 40, fontWeight: 500 }} autoFocus placeholder={t("Task name")} value={title} onChange={(e) => setTitle(e.target.value)} aria-label={t("Task name")} />
        <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: "0 16px", marginTop: 14 }}>
          <div className="field">
            <label htmlFor="task-status">{t("Status")}</label>
            <select id="task-status" className="select" value={status} onChange={(e) => setStatus(e.target.value as TaskStatus)}>
              <option value="todo">○ {t("To do")}</option>
              <option value="in_progress">◐ {t("In progress")}</option>
              <option value="done">● {t("Done")}</option>
            </select>
          </div>
          <div className="field">
            <label htmlFor="task-priority">{t("Priority")}</label>
            <select id="task-priority" className="select" value={priority} onChange={(e) => setPriority(e.target.value as TaskPriority)}>
              {(["none", "low", "medium", "high", "urgent"] as TaskPriority[]).map((p) => <option key={p} value={p}>{t(p[0].toUpperCase() + p.slice(1))}</option>)}
            </select>
          </div>
          <div className="field">
            <label htmlFor="task-due">{t("Due date")}</label>
            <input id="task-due" className="input" type="date" value={dueDate} onChange={(e) => setDueDate(e.target.value)} />
            <div className="row wrap" style={{ gap: 4 }}>
              {quick(t("Today"), today)}
              {quick(t("Tomorrow"), addDays(today, 1))}
              {quick(t("Next week"), addDays(today, 7))}
              {dueDate && <button type="button" className="btn small ghost" onClick={() => setDueDate("")}>{t("Clear")}</button>}
            </div>
          </div>
          <div className="field">
            <label htmlFor="task-remind">{t("Reminder")}</label>
            <input id="task-remind" className="input" type="datetime-local" value={remind} onChange={(e) => setRemind(e.target.value)} />
            <div className="row wrap" style={{ gap: 4 }}>
              {dueDate && <button type="button" className="btn small" onClick={() => setRemind(`${dueDate}T09:00`)}>{t("Due day 9:00")}</button>}
              {remind && <button type="button" className="btn small ghost" onClick={() => setRemind("")}>{t("No reminder")}</button>}
              <span className="small-text faint">{s.counts?.timezone}</span>
            </div>
          </div>
          <div className="field">
            <label htmlFor="task-project">{t("Project")}</label>
            <select id="task-project" className="select" value={projectId} onChange={(e) => setProjectId(e.target.value)}>
              <option value="">{t("No project")}</option>
              {projects.map((p) => <option key={p.id} value={p.id}>{p.icon ?? "📁"} {p.title || t("Untitled")}</option>)}
            </select>
          </div>
          <div className="field">
            <label>{t("Related pages")}</label>
            <div className="row wrap" style={{ gap: 4 }}>
              {task?.sourcePageId && (
                <span className="chip" title={t("Created from this page")}>📄 {task.sourcePageTitle || t("Untitled")}</span>
              )}
              {links.map((l) => {
                const p = s.pageById(l) ?? task?.links.find((x) => x.id === l);
                return (
                  <span key={l} className="chip">
                    <span role="link" onClick={() => (onClose(), s.navigate({ name: "page", id: l }))}>{p ? (p.icon || kindIcon(p.kind)) + " " + (p.title || t("Untitled")) : l.slice(0, 6)}</span>
                    <button type="button" aria-label={t("Remove")} onClick={() => setLinks(links.filter((x) => x !== l))}>×</button>
                  </span>
                );
              })}
            </div>
            <input className="input" placeholder={t("Link a page…")} value={linkQ} onChange={(e) => setLinkQ(e.target.value)} />
            {linkCandidates.map((p) => (
              <button type="button" key={p.id} className="menu-item" onClick={() => (setLinks([...links, p.id]), setLinkQ(""))}>
                {p.icon || kindIcon(p.kind, p.canvasKind)} {p.title || t("Untitled")}
              </button>
            ))}
          </div>
        </div>
        <button type="button" className="btn small ghost" onClick={() => setShowGrowth(!showGrowth)} aria-expanded={showGrowth} style={{ marginBottom: 8 }}>
          {showGrowth ? "▾" : "▸"} 🎯 {t("Goal, skills & effort")}
        </button>
        {showGrowth && (
          <div className="card-box" style={{ padding: "10px 14px" }}>
            <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr 1fr", gap: "0 12px" }}>
              <div className="field">
                <label>{t("Milestone")}</label>
                <select className="select" value={milestoneId} onChange={(e) => {
                  setMilestoneId(e.target.value);
                  const ms = goalOptions.flatMap((g) => g.milestones).find((m) => m.id === e.target.value);
                  if (ms?.requirementId && !reqLinks.length) setReqLinks([{ requirementId: ms.requirementId, allocationPct: 100 }]);
                }}>
                  <option value="">—</option>
                  {goalOptions.map((g) => <optgroup key={g.id} label={g.title}>{g.milestones.map((m) => <option key={m.id} value={m.id}>{m.phase ? `${m.phase} · ` : ""}{m.title}</option>)}</optgroup>)}
                </select>
              </div>
              <div className="field"><label>{t("Estimated effort (min)")}</label><input className="input" inputMode="numeric" value={estimated} onChange={(e) => setEstimated(e.target.value)} /></div>
              <div className="field"><label>{t("Actual effort (min)")}</label><input className="input" inputMode="numeric" value={actual} onChange={(e) => setActual(e.target.value)} /></div>
            </div>
            <div className="field">
              <label>{t("Contributes to requirements")} <span className="faint">({t("allocation of this task's effort, total ≤ 100%")})</span></label>
              {reqLinks.map((l, i) => (
                <div key={i} className="row" style={{ marginBottom: 4 }}>
                  <select className="select" value={l.requirementId} onChange={(e) => setReqLinks(reqLinks.map((x, j) => (j === i ? { ...x, requirementId: e.target.value } : x)))}>
                    {goalOptions.map((g) => <optgroup key={g.id} label={g.title}>{g.requirements.map((r) => <option key={r.id} value={r.id}>{r.title}</option>)}</optgroup>)}
                  </select>
                  <input className="input" style={{ width: 70 }} type="number" min={1} max={100} value={l.allocationPct} onChange={(e) => setReqLinks(reqLinks.map((x, j) => (j === i ? { ...x, allocationPct: Number(e.target.value) } : x)))} aria-label={t("Allocation %")} />%
                  <button type="button" className="icon-btn small" aria-label={t("Remove")} onClick={() => setReqLinks(reqLinks.filter((_, j) => j !== i))}>×</button>
                </div>
              ))}
              {reqLinks.reduce((a, l) => a + (l.allocationPct || 0), 0) > 100 && <div className="small-text" style={{ color: "var(--danger)" }}>{t("Allocations add up to more than 100%.")}</div>}
              {goalOptions.some((g) => g.requirements.length) && (
                <button type="button" className="btn small" onClick={() => {
                  const first = goalOptions.flatMap((g) => g.requirements).find((r) => !reqLinks.some((l) => l.requirementId === r.id));
                  if (first) setReqLinks([...reqLinks, { requirementId: first.id, allocationPct: Math.max(1, 100 - reqLinks.reduce((a, l) => a + l.allocationPct, 0)) }]);
                }}>＋ {t("Link requirement")}</button>
              )}
            </div>
            <div className="field">
              <label>{t("Practises skills")} <span className="faint">({t("practice is tracked; levels change only with evidence")})</span></label>
              <div className="row wrap" style={{ gap: 4 }}>
                {skillOptions.map((k) => (
                  <button type="button" key={k.id} className={`btn small${skillIds.includes(k.id) ? " primary" : ""}`} aria-pressed={skillIds.includes(k.id)}
                    onClick={() => setSkillIds(skillIds.includes(k.id) ? skillIds.filter((x) => x !== k.id) : [...skillIds, k.id])}>{k.name}</button>
                ))}
                {!skillOptions.length && <span className="small-text faint">{t("No skills yet — add them in Learning & skills.")}</span>}
              </div>
            </div>
            <div className="field">
              <label>{t("Depends on")}</label>
              <div className="row wrap" style={{ gap: 4 }}>
                {dependsOn.map((d) => <span key={d} className="chip">⛓ {openTasks.find((x) => x.id === d)?.title ?? task?.dependsOn.find((x) => x.id === d)?.title ?? d.slice(0, 6)}<button type="button" onClick={() => setDependsOn(dependsOn.filter((x) => x !== d))}>×</button></span>)}
                <select className="select" style={{ width: 240 }} value="" onChange={(e) => e.target.value && setDependsOn([...dependsOn, e.target.value])} aria-label={t("Add dependency")}>
                  <option value="">{t("Add a prerequisite task…")}</option>
                  {openTasks.filter((x) => x.id !== task?.id && !dependsOn.includes(x.id)).map((x) => <option key={x.id} value={x.id}>{x.title}</option>)}
                </select>
              </div>
            </div>
          </div>
        )}
        <div className="field">
          <label htmlFor="task-desc">{t("Details")}</label>
          <textarea id="task-desc" className="textarea" rows={4} value={description} onChange={(e) => setDescription(e.target.value)} />
        </div>
        {task?.sourcePageId && (
          <button type="button" className="btn small ghost" onClick={() => (onClose(), s.navigate({ name: "page", id: task.sourcePageId! }))}>
            📄 {t("Open source page")}
          </button>
        )}
        {task?.sourceCanvasId && (
          <button type="button" className="btn small ghost" onClick={() => (onClose(), s.navigate({ name: "page", id: task.sourceCanvasId!, tab: task.sourceElementId ? `el:${task.sourceElementId}` : undefined }))}>
            🎨 {t("Open on canvas")}
          </button>
        )}
      </form>
    </Modal>
  );
}
