import { useEffect, useState } from "react";
import { useStore } from "../../app/providers/store";
import { useI18n } from "../../app/providers/i18n";
import { api } from "../../services/desktop-bridge/api";
import type { PageMeta, Task } from "../../shared/types";
import { relativeTime } from "../../shared/utils";
import { kindIcon } from "../../components/ui/Icon";
import { TaskRow } from "./TaskRow";
import { TodayGrowth } from "../goals/TodayGrowth";

function greeting(t: (s: string) => string) {
  const h = new Date().getHours();
  return h < 12 ? t("Good morning") : h < 18 ? t("Good afternoon") : t("Good evening");
}

/** Home / Today: overdue + due today, missed reminders, recent pages and projects. */
export function HomeView() {
  const s = useStore();
  const { t, locale } = useI18n();
  const [today, setToday] = useState<Task[]>([]);
  const [overdue, setOverdue] = useState<Task[]>([]);
  const [missed, setMissed] = useState<Task[]>([]);
  const [recent, setRecent] = useState<PageMeta[]>([]);
  const load = () => {
    api.tasks.list("today").then(setToday).catch(s.fail);
    api.tasks.list("overdue").then(setOverdue).catch(s.fail);
    api.tasks.missed().then(setMissed).catch(() => {});
    api.pages.recent(8).then(setRecent).catch(() => {});
  };
  useEffect(load, [s.taskDialog, s.counts]); // eslint-disable-line react-hooks/exhaustive-deps

  const projects = s.tree.filter((p) => p.kind === "project" && !p.isTemplate).slice(0, 8);
  const dateLabel = new Date().toLocaleDateString(locale, { weekday: "long", day: "numeric", month: "long", year: "numeric", timeZone: s.counts?.timezone });

  return (
    <div className="content">
      <div className="page">
        <h1 style={{ margin: "24px 0 2px", fontSize: 30 }}>{greeting(t)}</h1>
        <div className="muted">{dateLabel}</div>

        {missed.length > 0 && (
          <div className="banner" style={{ marginTop: 18 }}>
            <span>⏰ {t("{n} reminder(s) came due while the app was closed or the Mac was asleep.", { n: missed.length })}</span>
            <span className="spacer" />
            <button className="btn small" onClick={() => api.tasks.dismissMissed().then(load)}>{t("Dismiss")}</button>
          </div>
        )}
        {missed.map((x) => <TaskRow key={`m-${x.id}`} task={x} onChanged={load} />)}

        <div className="h-section">
          <span>{t("Today")}</span>
          <span className="faint">{today.length + overdue.length}</span>
          <span className="spacer" />
          <button className="btn small ghost" onClick={() => s.openTask(undefined, { dueDate: s.counts?.date })}>＋ {t("Task")}</button>
          <button className="btn small ghost" onClick={() => s.navigate({ name: "tasks" })}>{t("All tasks")} →</button>
        </div>
        {overdue.map((x) => <TaskRow key={x.id} task={x} onChanged={() => (load(), s.refreshCounts())} />)}
        {today.map((x) => <TaskRow key={x.id} task={x} onChanged={() => (load(), s.refreshCounts())} />)}
        {!today.length && !overdue.length && <div className="muted" style={{ padding: "8px 0" }}>🌤️ {t("Nothing due today.")}</div>}

        <TodayGrowth />

        <div className="h-section">{t("Recently visited")}</div>
        {recent.length ? (
          <div className="home-grid">
            {recent.map((p) => (
              <div key={p.id} className="tile" onClick={() => s.navigate({ name: "page", id: p.id })} role="link" tabIndex={0} onKeyDown={(e) => e.key === "Enter" && s.navigate({ name: "page", id: p.id })}>
                <span className="tile-icon">{p.icon || kindIcon(p.kind, p.canvasKind)}</span>
                <span className="tile-title">{p.title || t("Untitled")}</span>
                <span className="small-text faint">{p.lastOpenedAt && relativeTime(p.lastOpenedAt, locale)}</span>
              </div>
            ))}
          </div>
        ) : (
          <div className="muted">{t("Pages you open show up here.")}</div>
        )}

        <div className="h-section">
          <span>{t("Projects")}</span>
          <span className="spacer" />
          <button className="btn small ghost" onClick={() => s.createPage({ kind: "project" })}>＋ {t("New project")}</button>
        </div>
        {projects.length ? (
          <div className="home-grid">
            {projects.map((p) => (
              <div key={p.id} className="tile" onClick={() => s.navigate({ name: "page", id: p.id })} role="link" tabIndex={0}>
                <span className="tile-icon">{p.icon || "📁"}</span>
                <span className="tile-title">{p.title || t("Untitled")}</span>
              </div>
            ))}
          </div>
        ) : (
          <div className="muted">{t("No projects yet. Create one, or start from the Project template.")}</div>
        )}
      </div>
    </div>
  );
}
