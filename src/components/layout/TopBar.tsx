import { useEffect, useState } from "react";
import { useStore } from "../../app/providers/store";
import { useI18n } from "../../app/providers/i18n";
import { api } from "../../services/desktop-bridge/api";
import type { PageMeta } from "../../shared/types";
import { Icon, kindIcon } from "../ui/Icon";
import { Menu, useAnchor } from "../ui/Popover";
import { usePageActions } from "../../features/pages/pageActions";
import { isMac } from "../../shared/utils";

const routeTitles: Record<string, string> = {
  home: "Home",
  tasks: "Tasks",
  search: "Search",
  trash: "Trash",
  archive: "Archive",
  templates: "Templates",
  settings: "Settings",
  goals: "Goals & Roadmaps",
  goal: "Goal",
  skills: "Learning & skills",
  habits: "Habits & routines",
  finance: "Personal finance",
  review: "Weekly review",
  form: "Form",
  automations: "Automations",
  timetable: "Timetable",
};

export function TopBar() {
  const s = useStore();
  const { t } = useI18n();
  const menu = useAnchor();
  const actions = usePageActions();
  const [extra, setExtra] = useState<PageMeta | null>(null);
  const pageId = s.route.name === "page" ? s.route.id : null;
  const inTree = s.pageById(pageId);

  // Records are not part of the sidebar tree; fetch their metadata for the breadcrumb.
  useEffect(() => {
    setExtra(null);
    if (pageId && !inTree) api.pages.meta(pageId).then(setExtra).catch(() => {});
  }, [pageId, inTree, s.tree]);

  const current = inTree ?? extra;
  const chain: PageMeta[] = [];
  let cur = current?.parentId ? s.pageById(current.parentId) : undefined;
  while (cur && chain.length < 20) {
    chain.unshift(cur);
    cur = cur.parentId ? s.pageById(cur.parentId) : undefined;
  }
  const section = s.sections.find((x) => x.id === (chain[0] ?? current)?.sectionId);

  const status = s.saveStatus;
  const mod = isMac ? "⌘" : "Ctrl+";

  return (
    <header className={`topbar${s.sidebarOpen ? "" : " no-sidebar"}`} data-tauri-drag-region>
      <button className="icon-btn" onClick={s.toggleSidebar} title={`${t("Toggle sidebar")} (${mod}\\)`} aria-label={t("Toggle sidebar")}><Icon name="sidebar" /></button>
      <button className="icon-btn" onClick={s.back} disabled={!s.canBack} title={`${t("Back")} (${mod}[)`} aria-label={t("Back")}><Icon name="chevronLeft" /></button>
      <button className="icon-btn" onClick={s.forward} disabled={!s.canForward} title={`${t("Forward")} (${mod}])`} aria-label={t("Forward")}><Icon name="chevronRight" /></button>
      <nav className="crumbs" aria-label={t("Breadcrumb")} data-tauri-drag-region>
        {s.route.name === "page" ? (
          <>
            {section && (
              <>
                <button className="crumb muted" onClick={() => s.navigate({ name: "section", id: section.id })}>{section.name}</button>
                <span className="crumb-sep">/</span>
              </>
            )}
            {chain.map((p) => (
              <span key={p.id} className="row" style={{ gap: 2, minWidth: 0 }}>
                <button className="crumb" onClick={() => s.navigate({ name: "page", id: p.id })}>{p.icon || kindIcon(p.kind, p.canvasKind)} {p.title || t("Untitled")}</button>
                <span className="crumb-sep">/</span>
              </span>
            ))}
            {current && <span className="crumb" aria-current="page">{current.icon || kindIcon(current.kind, current.canvasKind)} {current.title || t("Untitled")}</span>}
          </>
        ) : s.route.name === "section" ? (
          <span className="crumb">{s.sections.find((x) => x.id === (s.route as { id: string }).id)?.name}</span>
        ) : (
          <span className="crumb">{t(routeTitles[s.route.name] ?? "")}</span>
        )}
      </nav>
      {s.route.name === "page" && (
        <span className="small-text faint" aria-live="polite" style={{ marginRight: 6, color: status === "error" ? "var(--danger)" : undefined }}>
          {status === "saving" ? t("Saving…") : status === "saved" ? t("Saved") : status === "error" ? t("Not saved — retrying") : ""}
        </span>
      )}
      <button className="icon-btn" onClick={() => s.setPaletteOpen(true)} title={`${t("Search & commands")} (${mod}K)`} aria-label={t("Search & commands")}><Icon name="search" /></button>
      {s.route.name === "page" && (
        <>
          <button className={`icon-btn${s.inspectorOpen ? " on" : ""}`} onClick={s.toggleInspector} title={t("Toggle inspector")} aria-label={t("Toggle inspector")}><Icon name="inspector" /></button>
          {current && (
            <button className="icon-btn" onClick={menu.toggle} aria-label={t("Page actions")}><Icon name="more" /></button>
          )}
        </>
      )}
      {menu.anchor && current && <Menu anchor={menu.anchor} align="end" items={actions.menu(current)} onClose={menu.close} />}
    </header>
  );
}
