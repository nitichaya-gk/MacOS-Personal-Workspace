import { useEffect, useState, type ReactNode } from "react";
import { useStore } from "../../app/providers/store";
import { useI18n } from "../../app/providers/i18n";
import { api } from "../../services/desktop-bridge/api";
import type { AssetInfo, PageDetail, PageMeta } from "../../shared/types";
import { formatBytes, relativeTime } from "../../shared/utils";
import { kindIcon } from "../../components/ui/Icon";
import { EmptyState } from "../../components/ui/Modal";
import { BlockEditor } from "../block-editor/Editor";
import { Backlinks, PageHeader } from "../pages/PageView";
import { TaskList } from "../tasks/TasksView";
import { CanvasPreview } from "../diagrams/CanvasPreview";

type Tab = "overview" | "documents" | "tasks" | "diagrams" | "wireframes" | "assets";

/** A project page: overview content plus Documents / Tasks / Diagrams / Wireframes / Assets inside it. */
export function ProjectView({ detail, banners, onMeta }: { detail: PageDetail; banners: ReactNode; onMeta: (m: PageMeta) => void }) {
  const s = useStore();
  const { t, locale } = useI18n();
  const meta = detail.meta;
  const [tab, setTab] = useState<Tab>("overview");
  const [inside, setInside] = useState<PageMeta[]>([]);
  const [assets, setAssets] = useState<AssetInfo[]>([]);
  useEffect(() => {
    api.pages.subtree(meta.id).then(setInside).catch(s.fail);
    if (tab === "assets") api.assets.listFor(meta.id).then(setAssets).catch(s.fail);
  }, [meta.id, s.tree, tab, s.fail]);

  const docs = inside.filter((p) => p.kind === "page" || p.kind === "database" || p.kind === "project");
  const diagrams = inside.filter((p) => p.kind === "canvas" && p.canvasKind !== "wireframe");
  const wires = inside.filter((p) => p.kind === "canvas" && p.canvasKind === "wireframe");
  const tabs: { id: Tab; label: string; count?: number }[] = [
    { id: "overview", label: t("Overview") },
    { id: "documents", label: t("Documents"), count: docs.length },
    { id: "tasks", label: t("Tasks") },
    { id: "diagrams", label: t("Diagrams"), count: diagrams.length },
    { id: "wireframes", label: t("Wireframes"), count: wires.length },
    { id: "assets", label: t("Assets") },
  ];

  const list = (items: PageMeta[], empty: string, create: () => void, createLabel: string, previews = false) =>
    items.length ? (
      <>
        <div className="row" style={{ marginBottom: 8 }}><span className="spacer" /><button className="btn small primary" onClick={create}>＋ {createLabel}</button></div>
        {previews ? (
          <div className="home-grid" style={{ gridTemplateColumns: "repeat(auto-fill, minmax(240px, 1fr))" }}>
            {items.map((p) => (
              <div key={p.id} className="tile" onClick={() => s.navigate({ name: "page", id: p.id })} role="link" tabIndex={0}>
                <div className="cb-canvas-preview" style={{ height: 140 }}><CanvasPreview id={p.id} /></div>
                <span className="tile-title">{kindIcon("canvas", p.canvasKind)} {p.title || t("Untitled")}</span>
                <span className="small-text faint">{relativeTime(p.updatedAt, locale)}</span>
              </div>
            ))}
          </div>
        ) : (
          items.map((p) => (
            <div key={p.id} className="list-row" onClick={() => s.navigate({ name: "page", id: p.id })} role="link" tabIndex={0}>
              <span>{p.icon || kindIcon(p.kind, p.canvasKind)}</span>
              <span className="t">{p.title || t("Untitled")}</span>
              <span className="small-text faint">{relativeTime(p.updatedAt, locale)}</span>
            </div>
          ))
        )}
      </>
    ) : (
      <EmptyState icon="✨" title={empty}><button className="btn primary" onClick={create}>＋ {createLabel}</button></EmptyState>
    );

  return (
    <div className="page">
      {banners}
      <PageHeader meta={meta} onChange={onMeta} />
      <div className="tabs" role="tablist">
        {tabs.map((x) => (
          <button key={x.id} role="tab" aria-selected={tab === x.id} className={`tab${tab === x.id ? " active" : ""}`} onClick={() => setTab(x.id)}>
            {x.label} {!!x.count && <span className="count">{x.count}</span>}
          </button>
        ))}
      </div>
      {tab === "overview" && (
        <>
          <div className="editor-wrap"><BlockEditor page={meta} initialBlocks={detail.blocks} readOnly={!!meta.deletedAt} /></div>
          <Backlinks id={meta.id} />
        </>
      )}
      {tab === "documents" && list(docs, t("No documents in this project yet"), () => s.createPage({ kind: "page", parentId: meta.id }), t("New document"))}
      {tab === "tasks" && <TaskList view="open" projectId={meta.id} showProject={false} />}
      {tab === "diagrams" && list(diagrams, t("No diagrams yet"), () => s.createPage({ kind: "canvas", canvasKind: "diagram", parentId: meta.id }), t("New diagram"), true)}
      {tab === "wireframes" && list(wires, t("No wireframes yet"), () => s.createPage({ kind: "canvas", canvasKind: "wireframe", parentId: meta.id }), t("New wireframe"), true)}
      {tab === "assets" &&
        (assets.length ? (
          <table className="plain">
            <thead><tr><th>{t("File")}</th><th>{t("Type")}</th><th>{t("Size")}</th><th>{t("Page")}</th></tr></thead>
            <tbody>
              {assets.map((a) => (
                <tr key={a.id}>
                  <td>{a.mime.startsWith("image/") ? <img src={a.url} alt="" style={{ width: 32, height: 32, objectFit: "cover", borderRadius: 4, verticalAlign: "middle", marginRight: 8 }} /> : "📎 "}{a.fileName}</td>
                  <td className="muted">{a.mime}</td>
                  <td className="muted">{formatBytes(a.size)}</td>
                  <td>{a.pageId && <button className="crumb" onClick={() => s.navigate({ name: "page", id: a.pageId! })}>{s.pageById(a.pageId)?.title || t("Open")}</button>}</td>
                </tr>
              ))}
            </tbody>
          </table>
        ) : (
          <EmptyState icon="📎" title={t("No files yet")} hint={t("Images and files added to pages in this project are listed here.")} />
        ))}
    </div>
  );
}
