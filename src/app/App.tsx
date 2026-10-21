import { lazy, Suspense, useEffect, useRef } from "react";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { MantineProvider } from "@mantine/core";
import { StoreProvider, useColorScheme, useStore } from "./providers/store";
import { I18nProvider, useI18n } from "./providers/i18n";
import { Sidebar } from "../components/layout/Sidebar";
import { TopBar } from "../components/layout/TopBar";
import { CommandPalette } from "../components/layout/CommandPalette";
import { DialogHost, Toasts } from "../components/ui/Modal";
import { MoveDialogHost } from "../features/pages/pageActions";
import { HomeView } from "../features/tasks/HomeView";
import { TasksView } from "../features/tasks/TasksView";
import { TaskDialog } from "../features/tasks/TaskDialog";
import { PageView } from "../features/pages/PageView";
import { SearchView } from "../features/search/SearchView";
import { TrashView, ArchiveView, SectionView } from "../features/workspace/WorkspaceViews";
import { TemplatesView } from "../features/templates/TemplatesView";
import { isTauri } from "../services/desktop-bridge/api";

const SettingsView = lazy(() => import("../features/settings/SettingsView").then((m) => ({ default: m.SettingsView })));

function Routes() {
  const { route } = useStore();
  switch (route.name) {
    case "home":
      return <HomeView />;
    case "tasks":
      return <TasksView initialView={route.view} />;
    case "page":
      return <PageView key={route.id} id={route.id} />;
    case "search":
      return <SearchView initial={route.q} />;
    case "trash":
      return <TrashView />;
    case "archive":
      return <ArchiveView />;
    case "templates":
      return <TemplatesView />;
    case "section":
      return <SectionView key={route.id} id={route.id} />;
    case "settings":
      return (
        <Suspense fallback={<div className="loading">…</div>}>
          <SettingsView initialTab={route.tab} />
        </Suspense>
      );
  }
}

function useShortcuts() {
  const s = useStore();
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      const mod = e.metaKey || e.ctrlKey;
      if (!mod || e.isComposing) return;
      const k = e.key.toLowerCase();
      if (k === "k") (e.preventDefault(), s.setPaletteOpen(true));
      else if (k === "p" && !e.shiftKey) (e.preventDefault(), s.navigate({ name: "search" }));
      else if (k === "n" && !e.shiftKey) (e.preventDefault(), s.createPage({ kind: "page" }));
      else if (k === "t" && e.shiftKey) (e.preventDefault(), s.openTask());
      else if (k === "\\") (e.preventDefault(), s.toggleSidebar());
      else if (k === "[") (e.preventDefault(), s.back());
      else if (k === "]") (e.preventDefault(), s.forward());
      else if (k === ",") (e.preventDefault(), s.navigate({ name: "settings" }));
      else if (k === "i" && e.shiftKey) (e.preventDefault(), s.toggleInspector());
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [s]);
}

/** Reminder events from the Rust scheduler. Clicking a macOS notification focuses the app, which opens the task. */
function useReminderEvents() {
  const s = useStore();
  const { t } = useI18n();
  const last = useRef<{ taskId: string; at: number } | null>(null);
  const sRef = useRef(s);
  sRef.current = s;
  useEffect(() => {
    if (!isTauri()) return;
    const offs: Promise<() => void>[] = [
      listen<{ taskId: string; title: string }>("reminder-fired", (e) => {
        last.current = { taskId: e.payload.taskId, at: Date.now() };
        sRef.current.toast(`⏰ ${e.payload.title}`, { action: { label: t("Open"), run: () => sRef.current.openTask(e.payload.taskId) } });
        sRef.current.refreshCounts();
      }),
      listen<{ count: number }>("reminders-missed", (e) => {
        sRef.current.toast(t("You missed {n} reminder(s) while the app was closed.", { n: e.payload.count }), {
          action: { label: t("Review"), run: () => sRef.current.navigate({ name: "home" }) },
        });
      }),
      getCurrentWindow().onFocusChanged(({ payload: focused }) => {
        const l = last.current;
        if (focused && l && Date.now() - l.at < 3 * 60_000) {
          last.current = null;
          sRef.current.openTask(l.taskId);
        }
        if (focused) sRef.current.refreshCounts();
      }),
    ];
    return () => offs.forEach((p) => p.then((off) => off()));
  }, [t]);
}

function Shell() {
  const s = useStore();
  const { t } = useI18n();
  useShortcuts();
  useReminderEvents();
  if (s.fatal)
    return (
      <div className="error-box" role="alert">
        <strong>{t("The workspace could not be opened.")}</strong>
        <p className="mono">{s.fatal}</p>
        <button className="btn" onClick={() => location.reload()}>{t("Try again")}</button>
      </div>
    );
  if (!s.ready) return <div className="loading">{t("Opening workspace…")}</div>;
  return (
    <div className="app">
      {s.sidebarOpen && <Sidebar />}
      <div className="main">
        <TopBar />
        <div className="content-row">
          <Routes />
        </div>
      </div>
      {s.paletteOpen && <CommandPalette />}
      {s.taskDialog && <TaskDialog key={s.taskDialog.id ?? "new"} id={s.taskDialog.id} defaults={s.taskDialog.defaults} onClose={s.closeTask} />}
      <MoveDialogHost />
      <DialogHost />
      <Toasts />
    </div>
  );
}

function WithSettings() {
  const { settings } = useStore();
  const scheme = useColorScheme();
  useEffect(() => {
    document.documentElement.lang = settings?.language === "th" ? "th" : "en";
  }, [settings?.language]);
  return (
    <I18nProvider language={settings?.language}>
      <MantineProvider forceColorScheme={scheme}>
        <Shell />
      </MantineProvider>
    </I18nProvider>
  );
}

export function App() {
  return (
    <StoreProvider>
      <WithSettings />
    </StoreProvider>
  );
}
