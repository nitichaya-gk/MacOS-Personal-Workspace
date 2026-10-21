import { useEffect, useMemo, useRef, useState } from "react";
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import { useStore } from "../../app/providers/store";
import { useI18n } from "../../app/providers/i18n";
import { api } from "../../services/desktop-bridge/api";
import type { PageMeta, SearchHit } from "../../shared/types";
import { debounce } from "../../shared/utils";
import { kindIcon } from "../ui/Icon";
import { importMarkdownFile } from "../../features/pages/pageActions";

interface Item {
  key: string;
  icon: string;
  label: string;
  sub?: string;
  run: () => void;
}

export function CommandPalette() {
  const s = useStore();
  const { t } = useI18n();
  const [q, setQ] = useState("");
  const [sel, setSel] = useState(0);
  const [hits, setHits] = useState<SearchHit[]>([]);
  const [recent, setRecent] = useState<PageMeta[]>([]);
  const listRef = useRef<HTMLDivElement>(null);
  const close = () => s.setPaletteOpen(false);

  useEffect(() => {
    api.pages.recent(8).then(setRecent).catch(() => {});
  }, []);

  const runSearch = useMemo(
    () => debounce((query: string) => {
      if (!query.trim()) return setHits([]);
      api.search(query, { limit: 20 }).then(setHits).catch(() => setHits([]));
    }, 120),
    [],
  );
  useEffect(() => {
    runSearch(q);
    setSel(0);
  }, [q, runSearch]);

  const go = (id: string, kind: string) => (kind === "task" ? s.openTask(id) : s.navigate({ name: "page", id }));
  const commands: Item[] = [
    { key: "new-page", icon: "📄", label: t("New page"), run: () => s.createPage({ kind: "page" }) },
    { key: "new-project", icon: "📁", label: t("New project"), run: () => s.createPage({ kind: "project" }) },
    { key: "new-db", icon: "🗂️", label: t("New database"), run: () => s.createPage({ kind: "database" }) },
    { key: "new-diagram", icon: "🧭", label: t("New diagram"), run: () => s.createPage({ kind: "canvas", canvasKind: "diagram" }) },
    { key: "new-wire", icon: "📐", label: t("New wireframe"), run: () => s.createPage({ kind: "canvas", canvasKind: "wireframe" }) },
    { key: "new-task", icon: "✅", label: t("New task"), sub: "⌘⇧T", run: () => s.openTask() },
    { key: "home", icon: "🏠", label: t("Go to Home"), run: () => s.navigate({ name: "home" }) },
    { key: "tasks", icon: "☑️", label: t("Go to Tasks"), run: () => s.navigate({ name: "tasks" }) },
    { key: "search", icon: "🔎", label: t("Advanced search"), sub: "⌘P", run: () => s.navigate({ name: "search", q }) },
    { key: "templates", icon: "🧩", label: t("Go to Templates"), run: () => s.navigate({ name: "templates" }) },
    { key: "trash", icon: "🗑️", label: t("Go to Trash"), run: () => s.navigate({ name: "trash" }) },
    { key: "settings", icon: "⚙️", label: t("Settings"), sub: "⌘,", run: () => s.navigate({ name: "settings" }) },
    {
      key: "import-md", icon: "⬇️", label: t("Import Markdown…"),
      run: async () => {
        const f = await importMarkdownFile().catch((e) => (s.fail(e), null));
        if (!f) return;
        const p = await s.createPage({ kind: "page", title: f.name, open: false });
        if (p) {
          sessionStorage.setItem(`import:${p.id}`, f.text);
          s.navigate({ name: "page", id: p.id });
        }
      },
    },
    {
      key: "import-csv", icon: "⬇️", label: t("Import CSV as database…"),
      run: async () => {
        const path = await openDialog({ multiple: false, filters: [{ name: "CSV", extensions: ["csv"] }] });
        if (!path || Array.isArray(path)) return;
        try {
          const f = await api.exports.readText(path);
          const db = await api.databases.importCsv(f.text, f.name ?? "Imported");
          await s.refreshTree();
          s.navigate({ name: "page", id: db.id });
        } catch (e) {
          s.fail(e);
        }
      },
    },
    { key: "backup", icon: "💾", label: t("Back up now"), run: () => api.backup.create().then(() => s.toast(t("Backup created"), { kind: "success" })).catch(s.fail) },
    { key: "sidebar", icon: "◧", label: t("Toggle sidebar"), sub: "⌘\\", run: s.toggleSidebar },
  ];

  const query = q.trim().toLowerCase();
  const items: Item[] = [];
  if (!query) {
    items.push(...recent.map((p) => ({ key: `r-${p.id}`, icon: p.icon || kindIcon(p.kind, p.canvasKind), label: p.title || t("Untitled"), sub: t("Recent"), run: () => go(p.id, p.kind) })));
    items.push(...commands);
  } else {
    const titleMatches = s.tree.filter((p) => p.title.toLowerCase().includes(query)).slice(0, 8);
    const seen = new Set(titleMatches.map((p) => p.id));
    items.push(...titleMatches.map((p) => ({ key: `p-${p.id}`, icon: p.icon || kindIcon(p.kind, p.canvasKind), label: p.title || t("Untitled"), sub: t("Page"), run: () => go(p.id, p.kind) })));
    items.push(
      ...hits.filter((h) => !seen.has(h.entityId)).map((h) => ({
        key: `h-${h.entityId}`, icon: h.icon || kindIcon(h.entityType), label: h.title || t("Untitled"), sub: h.snippet, run: () => go(h.entityId, h.entityType),
      })),
    );
    items.push(...commands.filter((c) => c.label.toLowerCase().includes(query)));
  }

  useEffect(() => {
    listRef.current?.querySelector(".selected")?.scrollIntoView({ block: "nearest" });
  }, [sel]);

  return (
    <div className="overlay" onMouseDown={(e) => e.target === e.currentTarget && close()}>
      <div className="modal palette" role="dialog" aria-label={t("Search & commands")}>
        <input
          className="input"
          autoFocus
          placeholder={t("Search pages, tasks and commands…")}
          value={q}
          onChange={(e) => setQ(e.target.value)}
          onKeyDown={(e) => {
            if (e.nativeEvent.isComposing) return;
            if (e.key === "ArrowDown") (e.preventDefault(), setSel((i) => Math.min(items.length - 1, i + 1)));
            else if (e.key === "ArrowUp") (e.preventDefault(), setSel((i) => Math.max(0, i - 1)));
            else if (e.key === "Enter" && items[sel]) {
              e.preventDefault();
              close();
              items[sel].run();
            } else if (e.key === "Escape") close();
          }}
          aria-activedescendant={items[sel]?.key}
        />
        <div className="palette-list" ref={listRef} role="listbox">
          {items.length === 0 && <div className="muted" style={{ padding: 12 }}>{t("No results")}</div>}
          {items.map((it, i) => (
            <div
              key={it.key}
              id={it.key}
              role="option"
              aria-selected={i === sel}
              className={`palette-item${i === sel ? " selected" : ""}`}
              onMouseEnter={() => setSel(i)}
              onClick={() => {
                close();
                it.run();
              }}
            >
              <span style={{ width: 22, textAlign: "center" }}>{it.icon}</span>
              <div style={{ minWidth: 0, flex: 1 }}>
                <div className="main-text">{it.label}</div>
                {it.sub && <div className="sub">{it.sub}</div>}
              </div>
            </div>
          ))}
        </div>
      </div>
    </div>
  );
}
