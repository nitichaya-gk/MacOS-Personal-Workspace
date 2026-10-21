import { useMemo, useState, type DragEvent } from "react";
import { useStore } from "../../app/providers/store";
import { useI18n } from "../../app/providers/i18n";
import { api } from "../../services/desktop-bridge/api";
import type { PageKind, PageMeta, Section } from "../../shared/types";
import { clsx } from "../../shared/utils";
import { Icon, kindIcon } from "../ui/Icon";
import { Menu, useAnchor, type Anchor, type MenuEntry } from "../ui/Popover";
import { usePageActions } from "../../features/pages/pageActions";

function useExpanded() {
  const [open, setOpen] = useState<Set<string>>(() => {
    try {
      return new Set(JSON.parse(localStorage.getItem("expanded") ?? "[]"));
    } catch {
      return new Set();
    }
  });
  const toggle = (id: string, value?: boolean) =>
    setOpen((s) => {
      const n = new Set(s);
      if (value ?? !n.has(id)) n.add(id);
      else n.delete(id);
      try {
        localStorage.setItem("expanded", JSON.stringify([...n]));
      } catch {
        /* convenience only */
      }
      return n;
    });
  return { open, toggle };
}

type DropPos = "before" | "after" | "inside";

export function Sidebar() {
  const s = useStore();
  const { t } = useI18n();
  const { open, toggle } = useExpanded();
  const [drag, setDrag] = useState<string | null>(null);
  const [drop, setDrop] = useState<{ id: string; pos: DropPos } | null>(null);
  const menu = useAnchor();
  const [menuItems, setMenuItems] = useState<MenuEntry[]>([]);
  const actions = usePageActions();

  const children = useMemo(() => {
    const m = new Map<string, PageMeta[]>();
    for (const p of s.tree) {
      const key = p.parentId ?? `section:${p.sectionId}`;
      if (!m.has(key)) m.set(key, []);
      m.get(key)!.push(p);
    }
    m.forEach((list) => list.sort((a, b) => a.position - b.position));
    return m;
  }, [s.tree]);

  const activeId = s.route.name === "page" ? s.route.id : null;
  const favorites = s.tree.filter((p) => p.favorite);

  const showMenu = (anchor: Anchor, items: MenuEntry[]) => {
    setMenuItems(items);
    menu.open(anchor);
  };

  const addMenu = (anchor: Anchor, parentId: string | null, sectionId: string | null) =>
    showMenu(anchor, [
      { label: t("Page"), icon: <span>📄</span>, onSelect: () => newPage("page", parentId, sectionId) },
      { label: t("Project"), icon: <span>📁</span>, onSelect: () => newPage("project", parentId, sectionId) },
      { label: t("Database"), icon: <span>🗂️</span>, onSelect: () => newPage("database", parentId, sectionId) },
      { label: t("Diagram"), icon: <span>🧭</span>, onSelect: () => s.createPage({ kind: "canvas", canvasKind: "diagram", parentId, sectionId }).then(expandParent(parentId)) },
      { label: t("Wireframe"), icon: <span>📐</span>, onSelect: () => s.createPage({ kind: "canvas", canvasKind: "wireframe", parentId, sectionId }).then(expandParent(parentId)) },
      { separator: true },
      { label: t("From template…"), icon: <Icon name="template" />, onSelect: () => s.navigate({ name: "templates" }) },
    ]);

  const expandParent = (parentId: string | null) => () => parentId && toggle(parentId, true);
  const newPage = (kind: PageKind, parentId: string | null, sectionId: string | null) => s.createPage({ kind, parentId, sectionId }).then(expandParent(parentId));

  const sectionMenu = (anchor: Anchor, sec: Section) =>
    showMenu(anchor, [
      {
        label: t("Rename section"),
        onSelect: async () => {
          const name = await s.prompt({ title: t("Rename section"), defaultValue: sec.name });
          if (name?.trim()) await api.sections.update(sec.id, { name }).then(s.refreshTree).catch(s.fail);
        },
      },
      {
        label: t("Change icon"),
        onSelect: async () => {
          const icon = await s.prompt({ title: t("Section icon"), defaultValue: sec.icon ?? "", placeholder: "📁" });
          if (icon !== null) await api.sections.update(sec.id, { icon }).then(s.refreshTree).catch(s.fail);
        },
      },
      ...(sec.kind === "custom"
        ? [{ separator: true } as MenuEntry, { label: t("Delete section"), danger: true, onSelect: () => api.sections.remove(sec.id).then(s.refreshTree).catch(s.fail) } as MenuEntry]
        : []),
    ]);

  // ---------- drag and drop ----------
  const onDragOver = (e: DragEvent, target: PageMeta) => {
    if (!drag || drag === target.id) return;
    e.preventDefault();
    const r = (e.currentTarget as HTMLElement).getBoundingClientRect();
    const y = (e.clientY - r.top) / r.height;
    const pos: DropPos = y < 0.28 ? "before" : y > 0.72 ? "after" : target.kind === "database" ? "after" : "inside";
    if (drop?.id !== target.id || drop.pos !== pos) setDrop({ id: target.id, pos });
  };
  const doDrop = async (target: PageMeta) => {
    const id = drag;
    setDrag(null);
    setDrop(null);
    if (!id || !drop) return;
    try {
      if (drop.pos === "inside") {
        await api.pages.move(id, target.id, null);
        toggle(target.id, true);
      } else {
        const key = target.parentId ?? `section:${target.sectionId}`;
        const sibs = (children.get(key) ?? []).filter((p) => p.id !== id);
        const idx = sibs.findIndex((p) => p.id === target.id) + (drop.pos === "after" ? 1 : 0);
        await api.pages.move(id, target.parentId, target.sectionId, idx);
      }
      await s.refreshTree();
    } catch (e) {
      s.fail(e);
    }
  };
  const dropOnSection = async (sec: Section) => {
    const id = drag;
    setDrag(null);
    setDrop(null);
    if (!id) return;
    await api.pages.move(id, null, sec.id).then(s.refreshTree).catch(s.fail);
  };

  const renderTree = (key: string, depth: number) => {
    const list = children.get(key) ?? [];
    return list.map((p) => {
      const kids = children.get(p.id) ?? [];
      const expanded = open.has(p.id);
      return (
        <div key={p.id}>
          <div
            className={clsx("nav-item", activeId === p.id && "active", drop?.id === p.id && `drop-${drop.pos}`)}
            style={{ paddingLeft: 4 + depth * 14 }}
            draggable
            onDragStart={(e) => {
              setDrag(p.id);
              e.dataTransfer.effectAllowed = "move";
              e.dataTransfer.setData("text/plain", p.title);
            }}
            onDragEnd={() => {
              setDrag(null);
              setDrop(null);
            }}
            onDragOver={(e) => onDragOver(e, p)}
            onDragLeave={() => drop?.id === p.id && setDrop(null)}
            onDrop={(e) => {
              e.preventDefault();
              doDrop(p);
            }}
            onClick={(e) => (e.altKey ? s.openSplit(p.id) : s.navigate({ name: "page", id: p.id }))}
            title={t("⌥-click to open beside the current page")}
            onContextMenu={(e) => {
              e.preventDefault();
              showMenu({ x: e.clientX, y: e.clientY }, actions.menu(p, { onAddChild: () => toggle(p.id, true) }));
            }}
            role="treeitem"
            aria-expanded={kids.length ? expanded : undefined}
            aria-selected={activeId === p.id}
            tabIndex={0}
            onKeyDown={(e) => {
              if (e.key === "Enter") s.navigate({ name: "page", id: p.id });
              if (e.key === "ArrowRight") toggle(p.id, true);
              if (e.key === "ArrowLeft") toggle(p.id, false);
            }}
          >
            <button
              className="twisty"
              aria-label={expanded ? t("Collapse") : t("Expand")}
              onClick={(e) => {
                e.stopPropagation();
                toggle(p.id);
              }}
              style={{ visibility: kids.length || p.kind === "project" ? "visible" : "hidden" }}
            >
              <Icon name={expanded ? "chevronDown" : "chevronRight"} size={12} />
            </button>
            <span className="nav-icon" aria-hidden>{p.icon || kindIcon(p.kind, p.canvasKind)}</span>
            <span className="label">{p.title || t("Untitled")}</span>
            <span className="hover-actions">
              <button
                className="icon-btn small"
                aria-label={t("Page actions")}
                onClick={(e) => {
                  e.stopPropagation();
                  showMenu(e.currentTarget, actions.menu(p, { onAddChild: () => toggle(p.id, true) }));
                }}
              >
                <Icon name="more" size={14} />
              </button>
              {p.kind !== "database" && (
                <button
                  className="icon-btn small"
                  aria-label={t("Add inside")}
                  onClick={(e) => {
                    e.stopPropagation();
                    addMenu(e.currentTarget, p.id, null);
                  }}
                >
                  <Icon name="plus" size={14} />
                </button>
              )}
            </span>
          </div>
          {expanded && (kids.length ? renderTree(p.id, depth + 1) : <div className="empty-hint" style={{ paddingLeft: 34 + depth * 14 }}>{t("No pages inside")}</div>)}
        </div>
      );
    });
  };

  const navItem = (label: string, icon: string, active: boolean, onClick: () => void, badge?: number, alert?: boolean) => (
    <button className={clsx("nav-item", active && "active")} onClick={onClick}>
      <span className="nav-icon"><Icon name={icon} /></span>
      <span className="label">{label}</span>
      {!!badge && <span className={clsx("badge", alert && "alert")}>{badge}</span>}
    </button>
  );

  return (
    <nav className="sidebar" aria-label={t("Sidebar")}>
      <div className="sidebar-drag" data-tauri-drag-region />
      <div className="sidebar-scroll" role="tree">
        <button className="nav-item" style={{ fontWeight: 600, marginBottom: 4 }} onClick={() => s.navigate({ name: "settings", tab: "workspace" })}>
          <span className="nav-icon">🗄️</span>
          <span className="label">{s.info?.name ?? "Workspace"}</span>
        </button>
        {navItem(t("Search"), "search", s.route.name === "search", () => s.setPaletteOpen(true))}
        {navItem(t("Home"), "home", s.route.name === "home", () => s.navigate({ name: "home" }), s.counts?.today)}
        {navItem(t("Tasks"), "task", s.route.name === "tasks", () => s.navigate({ name: "tasks" }), s.counts?.overdue, true)}
        {navItem(t("Timetable"), "calendar", s.route.name === "timetable", () => s.navigate({ name: "timetable" }))}
        {navItem(t("Automations"), "settings", s.route.name === "automations", () => s.navigate({ name: "automations" }))}
        <div className="section-head"><span className="label">{t("Growth")}</span></div>
        {navItem(t("Goals"), "star", s.route.name === "goals" || s.route.name === "goal", () => s.navigate({ name: "goals" }))}
        {navItem(t("Learning hub"), "page", s.route.name === "learning", () => s.navigate({ name: "learning" }))}
        {navItem(t("Skills & evidence"), "template", s.route.name === "skills", () => s.navigate({ name: "skills" }))}
        {navItem(t("Habits"), "restore", s.route.name === "habits", () => s.navigate({ name: "habits" }))}
        {navItem(t("Finance"), "database", s.route.name === "finance", () => s.navigate({ name: "finance" }))}
        {navItem(t("Weekly review"), "calendar", s.route.name === "review", () => s.navigate({ name: "review" }))}

        {favorites.length > 0 && (
          <>
            <div className="section-head"><span className="label">{t("Favorites")}</span></div>
            {favorites.map((p) => (
              <button key={p.id} className={clsx("nav-item", activeId === p.id && "active")} onClick={() => s.navigate({ name: "page", id: p.id })}>
                <span className="nav-icon">{p.icon || kindIcon(p.kind, p.canvasKind)}</span>
                <span className="label">{p.title || t("Untitled")}</span>
              </button>
            ))}
          </>
        )}

        {s.sections.map((sec) => (
          <div key={sec.id}>
            <div
              className={clsx("section-head", drag && "drop-zone")}
              onDragOver={(e) => drag && e.preventDefault()}
              onDrop={(e) => {
                e.preventDefault();
                dropOnSection(sec);
              }}
              onContextMenu={(e) => {
                e.preventDefault();
                sectionMenu({ x: e.clientX, y: e.clientY }, sec);
              }}
            >
              <span className="label" onClick={() => s.navigate({ name: "section", id: sec.id })}>{sec.icon ? `${sec.icon} ` : ""}{sec.name}</span>
              <span className="hover-actions">
                <button className="icon-btn small" aria-label={t("Section actions")} onClick={(e) => sectionMenu(e.currentTarget, sec)}><Icon name="more" size={14} /></button>
                <button
                  className="icon-btn small"
                  aria-label={t("Add to section")}
                  onClick={(e) => {
                    if (sec.kind === "projects") newPage("project", null, sec.id);
                    else if (sec.kind === "databases") newPage("database", null, sec.id);
                    else addMenu(e.currentTarget, null, sec.id);
                  }}
                >
                  <Icon name="plus" size={14} />
                </button>
              </span>
            </div>
            {renderTree(`section:${sec.id}`, 0)}
            {!(children.get(`section:${sec.id}`)?.length) && <div className="empty-hint" style={{ paddingLeft: 12 }}>{t("Empty")}</div>}
          </div>
        ))}
        <button
          className="nav-item faint"
          style={{ marginTop: 10, color: "var(--text-muted)" }}
          onClick={async () => {
            const name = await s.prompt({ title: t("New section"), placeholder: t("e.g. Personal, Learning") });
            if (name?.trim()) await api.sections.create(name).then(s.refreshTree).catch(s.fail);
          }}
        >
          <span className="nav-icon"><Icon name="plus" /></span>
          <span className="label">{t("New section")}</span>
        </button>
      </div>
      <div className="sidebar-footer">
        <button className={clsx("icon-btn", s.route.name === "templates" && "on")} title={t("Templates")} aria-label={t("Templates")} onClick={() => s.navigate({ name: "templates" })}><Icon name="template" /></button>
        <button className={clsx("icon-btn", s.route.name === "archive" && "on")} title={t("Archive")} aria-label={t("Archive")} onClick={() => s.navigate({ name: "archive" })}><Icon name="archive" /></button>
        <button className={clsx("icon-btn", s.route.name === "trash" && "on")} title={t("Trash")} aria-label={t("Trash")} onClick={() => s.navigate({ name: "trash" })}><Icon name="trash" /></button>
        <span className="spacer" />
        <button className={clsx("icon-btn", s.route.name === "settings" && "on")} title={t("Settings")} aria-label={t("Settings")} onClick={() => s.navigate({ name: "settings" })}><Icon name="settings" /></button>
      </div>
      {menu.anchor && <Menu anchor={menu.anchor} items={menuItems} onClose={menu.close} />}
    </nav>
  );
}
