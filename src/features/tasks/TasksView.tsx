import { useEffect, useState } from "react";
import { useStore } from "../../app/providers/store";
import { useI18n } from "../../app/providers/i18n";
import { api } from "../../services/desktop-bridge/api";
import type { Task } from "../../shared/types";
import { EmptyState } from "../../components/ui/Modal";
import { TaskRow } from "./TaskRow";

const VIEWS = [
  { id: "today", label: "Today" },
  { id: "overdue", label: "Overdue" },
  { id: "upcoming", label: "Upcoming" },
  { id: "nodate", label: "No date" },
  { id: "completed", label: "Completed" },
] as const;

/** Task list with quick add. Used as the Tasks page and inside projects. */
export function TaskList({ view, projectId, showProject = true }: { view: string; projectId?: string; showProject?: boolean }) {
  const s = useStore();
  const { t } = useI18n();
  const [tasks, setTasks] = useState<Task[] | null>(null);
  const [quick, setQuick] = useState("");
  const load = () => api.tasks.list(view, { projectId, limit: 500 }).then(setTasks).catch(s.fail);
  useEffect(() => {
    load();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [view, projectId, s.taskDialog, s.counts]);

  const add = async () => {
    if (!quick.trim()) return;
    const dueDate = view === "today" || view === "overdue" ? s.counts?.date : undefined;
    await api.tasks.create({ title: quick.trim(), dueDate, projectId }).then(() => (setQuick(""), s.refreshCounts())).catch(s.fail);
  };

  return (
    <div>
      {view !== "completed" && (
        <input
          className="input"
          style={{ marginBottom: 8 }}
          placeholder={view === "today" || view === "overdue" ? t("Add a task for today and press Enter") : t("Add a task and press Enter")}
          value={quick}
          onChange={(e) => setQuick(e.target.value)}
          onKeyDown={(e) => e.key === "Enter" && !e.nativeEvent.isComposing && add()}
          aria-label={t("Quick add task")}
        />
      )}
      {tasks === null ? (
        <div className="loading">…</div>
      ) : tasks.length === 0 ? (
        <EmptyState icon={view === "today" ? "🌤️" : "✅"} title={view === "today" ? t("Nothing due today") : t("No tasks here")} hint={t("Tasks with a due date appear in Today; reminders notify you through macOS.")} />
      ) : (
        tasks.map((x) => <TaskRow key={x.id} task={x} onChanged={() => (load(), s.refreshCounts())} showProject={showProject} />)
      )}
    </div>
  );
}

export function TasksView({ initialView }: { initialView?: string }) {
  const s = useStore();
  const { t } = useI18n();
  const [view, setView] = useState(initialView ?? "today");
  const count = (id: string) => (id === "today" ? s.counts?.today : id === "overdue" ? s.counts?.overdue : id === "upcoming" ? s.counts?.upcoming : undefined);
  return (
    <div className="content">
      <div className="page">
        <div className="row" style={{ marginTop: 16 }}>
          <h1 style={{ margin: 0, fontSize: 30 }}>✅ {t("Tasks")}</h1>
          <span className="spacer" />
          <button className="btn primary" onClick={() => s.openTask(undefined, view === "today" ? { dueDate: s.counts?.date } : {})}>＋ {t("New task")}</button>
        </div>
        <p className="muted small-text">{t("Dates follow your time zone: {tz}", { tz: s.counts?.timezone ?? "" })}</p>
        <div className="tabs" role="tablist">
          {VIEWS.map((v) => (
            <button key={v.id} role="tab" aria-selected={view === v.id} className={`tab${view === v.id ? " active" : ""}`} onClick={() => setView(v.id)}>
              {t(v.label)} {!!count(v.id) && <span className="count">{count(v.id)}</span>}
            </button>
          ))}
        </div>
        <TaskList view={view} />
      </div>
    </div>
  );
}
