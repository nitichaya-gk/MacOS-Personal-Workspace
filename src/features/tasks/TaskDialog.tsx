import { useEffect, useState } from "react";
import { isPermissionGranted, requestPermission } from "@tauri-apps/plugin-notification";
import { useStore } from "../../app/providers/store";
import { useI18n } from "../../app/providers/i18n";
import { api, isTauri } from "../../services/desktop-bridge/api";
import type { Task, TaskPriority, TaskStatus } from "../../shared/types";
import { addDays } from "../../shared/utils";
import { Modal } from "../../components/ui/Modal";
import { kindIcon } from "../../components/ui/Icon";

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
        <div className="field">
          <label htmlFor="task-desc">{t("Details")}</label>
          <textarea id="task-desc" className="textarea" rows={4} value={description} onChange={(e) => setDescription(e.target.value)} />
        </div>
        {task?.sourcePageId && (
          <button type="button" className="btn small ghost" onClick={() => (onClose(), s.navigate({ name: "page", id: task.sourcePageId! }))}>
            📄 {t("Open source page")}
          </button>
        )}
      </form>
    </Modal>
  );
}
