import { useEffect, useMemo, useState, type ReactElement } from "react";
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import { useStore } from "../../app/providers/store";
import { useI18n } from "../../app/providers/i18n";
import { api, rpc } from "../../services/desktop-bridge/api";
import type { PageMeta } from "../../shared/types";
import { kindIcon } from "../../components/ui/Icon";
import { Modal } from "../../components/ui/Modal";

/** Opened by the `pw:publish` event (optionally with a page id to preselect). */
export function SiteExportHost() {
  const [initial, setInitial] = useState<string[] | null>(null);
  useEffect(() => {
    const on = (e: Event) => setInitial((e as CustomEvent<string | undefined>).detail ? [(e as CustomEvent<string>).detail] : []);
    window.addEventListener("pw:publish", on);
    return () => window.removeEventListener("pw:publish", on);
  }, []);
  return initial ? <SiteExportDialog initial={initial} onClose={() => setInitial(null)} /> : null;
}

function SiteExportDialog({ initial, onClose }: { initial: string[]; onClose: () => void }) {
  const s = useStore();
  const { t } = useI18n();
  const [picked, setPicked] = useState<Set<string>>(new Set(initial));
  const [children, setChildren] = useState(true);
  const [title, setTitle] = useState(s.info?.name ?? "My site");
  const [busy, setBusy] = useState(false);
  const roots = useMemo(() => s.tree.filter((p) => !p.isTemplate && !s.sections.find((x) => x.id === p.sectionId && x.kind === "templates")), [s.tree, s.sections]);
  const childrenOf = (id: string | null) => roots.filter((p) => p.parentId === id);
  const toggle = (id: string) => {
    const n = new Set(picked);
    if (n.has(id)) n.delete(id);
    else n.add(id);
    setPicked(n);
  };
  const row = (p: PageMeta, depth: number): ReactElement => (
    <div key={p.id}>
      <label className="menu-item" style={{ paddingLeft: 10 + depth * 18 }}>
        <input type="checkbox" checked={picked.has(p.id)} onChange={() => toggle(p.id)} /> {p.icon || kindIcon(p.kind, p.canvasKind)} {p.title || t("Untitled")}
      </label>
      {childrenOf(p.id).map((c) => row(c, depth + 1))}
    </div>
  );
  const publish = async () => {
    const dest = await openDialog({ directory: true, title: t("Choose where to save the website folder") });
    if (typeof dest !== "string") return;
    setBusy(true);
    try {
      // Canvases are rendered to SVG here (the drawing engine runs in the UI).
      const included = new Set<string>();
      const walk = (id: string) => {
        included.add(id);
        if (children) roots.filter((x) => x.parentId === id).forEach((x) => walk(x.id));
      };
      picked.forEach(walk);
      const svgs: Record<string, string> = {};
      const canvases = roots.filter((p) => p.kind === "canvas" && included.has(p.id));
      if (canvases.length) {
        const ex = await import("@excalidraw/excalidraw");
        for (const c of canvases) {
          const data = await api.canvases.load(c.id);
          if (!data.elements.length) continue;
          const svg = await ex.exportToSvg({ elements: data.elements as never, appState: { exportBackground: false, exportWithDarkMode: false } as never, files: Object.fromEntries(data.files.map((f) => [f.id, f])) as never, exportPadding: 16 });
          svgs[c.id] = svg.outerHTML;
        }
      }
      const r = await rpc<{ path: string; index: string; pages: number; assets: number }>("site.export", { pageIds: [...picked], includeChildren: children, title, dest, svgs });
      s.toast(t("Website exported: {n} pages", { n: r.pages }), { kind: "success", action: { label: t("Show in Finder"), run: () => api.system.reveal(r.index) } });
      onClose();
    } catch (e) {
      s.fail(e);
    } finally {
      setBusy(false);
    }
  };
  return (
    <Modal title={t("Publish as a static website")} wide onClose={onClose} footer={<><button className="btn" onClick={onClose}>{t("Cancel")}</button><button className="btn primary" disabled={!picked.size || busy} onClick={publish}>{busy ? "…" : t("Export website…")}</button></>}>
      <p className="small-text muted" style={{ marginTop: 0 }}>{t("Creates a folder of plain HTML files you can open in a browser or upload to any static host. Only the pages you select are included; links to other pages become plain text and only their attachments are copied.")}</p>
      <div className="field"><label>{t("Site title")}</label><input className="input" value={title} onChange={(e) => setTitle(e.target.value)} /></div>
      <label className="row small-text" style={{ marginBottom: 8 }}><input type="checkbox" checked={children} onChange={(e) => setChildren(e.target.checked)} /> {t("Include subpages of selected pages")}</label>
      <div style={{ maxHeight: 320, overflowY: "auto", border: "1px solid var(--border)", borderRadius: 8, padding: 4 }}>
        {s.sections.filter((x) => x.kind !== "templates").map((sec) => (
          <div key={sec.id}>
            <div className="menu-label">{sec.icon} {sec.name}</div>
            {roots.filter((p) => !p.parentId && p.sectionId === sec.id).map((p) => row(p, 0))}
          </div>
        ))}
      </div>
    </Modal>
  );
}
