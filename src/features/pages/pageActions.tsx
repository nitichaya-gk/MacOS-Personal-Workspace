import { useEffect, useMemo, useState } from "react";
import { save, open as openDialog } from "@tauri-apps/plugin-dialog";
import { useStore } from "../../app/providers/store";
import { useI18n } from "../../app/providers/i18n";
import { api } from "../../services/desktop-bridge/api";
import type { PageMeta } from "../../shared/types";
import { fileSafeName } from "../../shared/utils";
import { Icon, kindIcon } from "../../components/ui/Icon";
import { Modal } from "../../components/ui/Modal";
import type { MenuEntry } from "../../components/ui/Popover";

/** Shared page operations used by the sidebar, top bar and lists. */
export function usePageActions() {
  const s = useStore();
  const { t } = useI18n();

  const rename = async (p: PageMeta) => {
    const title = await s.prompt({ title: t("Rename"), defaultValue: p.title });
    if (title !== null) await api.pages.update(p.id, { title }).then(s.refreshTree).catch(s.fail);
  };

  const trash = async (p: PageMeta) => {
    try {
      await api.pages.trashPage(p.id);
      await s.refreshTree();
      if (s.route.name === "page" && s.route.id === p.id) s.navigate({ name: "home" }, { replace: true });
      s.toast(t("Moved “{title}” to Trash", { title: p.title || t("Untitled") }), {
        action: { label: t("Undo"), run: () => api.pages.restore(p.id).then(s.refreshTree).catch(s.fail) },
      });
    } catch (e) {
      s.fail(e);
    }
  };

  const exportMarkdown = async (p: PageMeta) => {
    const dest = await save({ defaultPath: `${fileSafeName(p.title)}.md`, filters: [{ name: "Markdown", extensions: ["md"] }] });
    if (!dest) return;
    try {
      const r = await api.exports.pageMarkdown(p.id, dest);
      s.toast(r.warnings.length ? `${t("Exported")} — ${r.warnings.join(" ")}` : t("Exported to {path}", { path: r.path }), { kind: "success" });
    } catch (e) {
      s.fail(e);
    }
  };

  const exportPdf = async (p: PageMeta) => {
    if (s.route.name !== "page" || s.route.id !== p.id) s.navigate({ name: "page", id: p.id });
    s.toast(t("Choose “Save as PDF” in the print dialog."));
    setTimeout(() => api.system.print().catch(s.fail), 400);
  };

  const menu = (p: PageMeta, opts: { onAddChild?: () => void } = {}): MenuEntry[] => [
    { label: t("Open"), icon: <Icon name="external" />, onSelect: () => s.navigate({ name: "page", id: p.id }) },
    { label: t("Rename"), icon: <Icon name="page" />, onSelect: () => rename(p) },
    {
      label: p.favorite ? t("Remove from Favorites") : t("Add to Favorites"),
      icon: <Icon name="star" />,
      onSelect: () => api.pages.update(p.id, { favorite: !p.favorite }).then(s.refreshTree).catch(s.fail),
    },
    ...(p.kind !== "database"
      ? [{
          label: t("Add subpage"),
          icon: <Icon name="plus" />,
          onSelect: () => s.createPage({ kind: "page", parentId: p.id }).then(() => opts.onAddChild?.()),
        } as MenuEntry]
      : []),
    { label: t("Duplicate"), icon: <Icon name="copy" />, onSelect: () => api.pages.duplicate(p.id).then(s.refreshTree).catch(s.fail) },
    { label: t("Move to…"), icon: <Icon name="folder" />, onSelect: () => window.dispatchEvent(new CustomEvent("pw:move", { detail: p })) },
    { label: t("Save as template"), icon: <Icon name="template" />, onSelect: () => api.templates.saveAs(p.id).then(() => { s.refreshTree(); s.toast(t("Saved to Templates"), { kind: "success" }); }).catch(s.fail) },
    { separator: true },
    ...(p.kind !== "canvas"
      ? [
          { label: t("Export Markdown…"), icon: <Icon name="download" />, onSelect: () => exportMarkdown(p) } as MenuEntry,
          { label: t("Export PDF…"), icon: <Icon name="print" />, onSelect: () => exportPdf(p) } as MenuEntry,
        ]
      : []),
    { label: t("Archive"), icon: <Icon name="archive" />, onSelect: () => api.pages.archive(p.id).then(() => { s.refreshTree(); s.toast(t("Archived")); }).catch(s.fail) },
    { label: t("Move to Trash"), icon: <Icon name="trash" />, danger: true, onSelect: () => trash(p) },
  ];

  return { menu, rename, trash, exportMarkdown, exportPdf };
}

/** "Move to…" picker: any section root or any page outside the moved page's own subtree. */
export function MoveDialogHost() {
  const s = useStore();
  const { t } = useI18n();
  const [page, setPage] = useState<PageMeta | null>(null);
  const [q, setQ] = useState("");
  useEffect(() => {
    const on = (e: Event) => {
      setPage((e as CustomEvent<PageMeta>).detail);
      setQ("");
    };
    window.addEventListener("pw:move", on);
    return () => window.removeEventListener("pw:move", on);
  }, []);

  const blocked = useMemo(() => {
    if (!page) return new Set<string>();
    const out = new Set([page.id]);
    let grew = true;
    while (grew) {
      grew = false;
      for (const p of s.tree) if (p.parentId && out.has(p.parentId) && !out.has(p.id)) (out.add(p.id), (grew = true));
    }
    return out;
  }, [page, s.tree]);

  if (!page) return null;
  const query = q.trim().toLowerCase();
  const targets = s.tree.filter((p) => !blocked.has(p.id) && p.kind !== "database" && (!query || p.title.toLowerCase().includes(query))).slice(0, 60);
  const move = async (parentId: string | null, sectionId: string | null) => {
    try {
      await api.pages.move(page.id, parentId, sectionId);
      await s.refreshTree();
      setPage(null);
      s.toast(t("Moved"), { kind: "success" });
    } catch (e) {
      s.fail(e);
    }
  };
  return (
    <Modal title={t("Move “{title}” to…", { title: page.title || t("Untitled") })} onClose={() => setPage(null)}>
      <input className="input" autoFocus placeholder={t("Search pages")} value={q} onChange={(e) => setQ(e.target.value)} />
      <div style={{ marginTop: 8, maxHeight: 360, overflowY: "auto" }}>
        <div className="menu-label">{t("Sections")}</div>
        {s.sections.filter((x) => !query || x.name.toLowerCase().includes(query)).map((x) => (
          <button key={x.id} className="menu-item" onClick={() => move(null, x.id)}>
            <span>{x.icon ?? "📁"}</span> {x.name}
          </button>
        ))}
        <div className="menu-label">{t("Pages")}</div>
        {targets.map((p) => (
          <button key={p.id} className="menu-item" onClick={() => move(p.id, null)}>
            <span>{p.icon || kindIcon(p.kind, p.canvasKind)}</span> {p.title || t("Untitled")}
          </button>
        ))}
      </div>
    </Modal>
  );
}

export async function importMarkdownFile(): Promise<{ text: string; name: string } | null> {
  const path = await openDialog({ multiple: false, filters: [{ name: "Markdown", extensions: ["md", "markdown", "txt"] }] });
  if (!path || Array.isArray(path)) return null;
  const r = await api.exports.readText(path);
  return { text: r.text, name: r.name ?? "Imported" };
}
