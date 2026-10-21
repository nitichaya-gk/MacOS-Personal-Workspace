import { useStore } from "../../app/providers/store";
import { useI18n } from "../../app/providers/i18n";
import { api } from "../../services/desktop-bridge/api";
import type { Task, TaskStatus } from "../../shared/types";
import { formatDate } from "../../shared/utils";

const NEXT: Record<TaskStatus, TaskStatus> = { todo: "done", in_progress: "done", done: "todo" };

/** Status toggle; the glyph (not only colour) shows the state. */
export function TaskCheck({ task, onChange }: { task: Task; onChange?: (t: Task) => void }) {
  const s = useStore();
  const { t } = useI18n();
  const label = task.status === "done" ? t("Done") : task.status === "in_progress" ? t("In progress") : t("To do");
  return (
    <button
      className={`check ${task.status}`}
      aria-label={`${label} — ${t("toggle")}`}
      title={label}
      onClick={(e) => {
        e.stopPropagation();
        api.tasks.update(task.id, { status: NEXT[task.status] }).then((x) => (onChange?.(x), s.refreshCounts())).catch(s.fail);
      }}
    >
      {task.status === "done" ? "✓" : ""}
    </button>
  );
}

export function TaskRow({ task, compact, onChanged, showProject = true }: { task: Task; compact?: boolean; onChanged?: () => void; showProject?: boolean }) {
  const s = useStore();
  const { t, locale } = useI18n();
  const today = s.counts?.date ?? "";
  const overdue = task.status !== "done" && !!task.dueDate && task.dueDate < today;
  return (
    <div className={`task-row ${task.status}`} onClick={() => s.openTask(task.id)} role="button" tabIndex={0} onKeyDown={(e) => e.key === "Enter" && s.openTask(task.id)}
      style={compact ? { padding: "5px 2px" } : undefined}>
      <TaskCheck task={task} onChange={() => onChanged?.()} />
      <div className="t">
        <div className="name">{task.title}</div>
        {!compact && (
          <div className="task-meta">
            {task.dueDate && <span className={overdue ? "overdue" : ""}>{overdue ? "⚠︎ " : "📅 "}{task.dueDate === today ? t("Today") : formatDate(task.dueDate, locale)}</span>}
            {task.remindAtLocal && <span>⏰ {task.remindAtLocal.replace("T", " ")}</span>}
            {showProject && task.projectTitle && <span>📁 {task.projectTitle}</span>}
            {task.sourcePageTitle && <span>📄 {task.sourcePageTitle}</span>}
            {task.status === "in_progress" && <span>◐ {t("In progress")}</span>}
          </div>
        )}
      </div>
      {task.priority !== "none" && <span className={`prio ${task.priority}`}>{t(task.priority[0].toUpperCase() + task.priority.slice(1))}</span>}
    </div>
  );
}
