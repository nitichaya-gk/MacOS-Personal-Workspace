import { useEffect, useState } from "react";
import { useStore } from "../../app/providers/store";
import { useI18n } from "../../app/providers/i18n";
import { api } from "../../services/desktop-bridge/api";
import type { PageMeta, Task } from "../../shared/types";
import { relativeTime } from "../../shared/utils";
import { kindIcon } from "../../components/ui/Icon";
import { EmptyState } from "../../components/ui/Modal";

export function TrashView() {
  const s = useStore();
  const { t, locale } = useI18n();
  const [data, setData] = useState<{ pages: PageMeta[]; tasks: Task[] } | null>(null);
  const load = () => api.pages.trash().then(setData).catch(s.fail);
  useEffect(() => {
    load();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);
  const after = () => (load(), s.refreshTree(), s.refreshCounts());

  const forever = async (title: string, run: () => Promise<unknown>) => {
    if (await s.confirm({ title: t("Delete “{title}” forever?", { title: title || t("Untitled") }), message: t("This cannot be undone. Backups made earlier still contain it."), danger: true, confirmLabel: t("Delete forever") }))
      run().then(after).catch(s.fail);
  };

  const empty = !data || (!data.pages.length && !data.tasks.length);
  return (
    <div className="content">
      <div className="page">
        <div className="row" style={{ marginTop: 16 }}>
          <h1 style={{ margin: 0, fontSize: 30 }}>🗑️ {t("Trash")}</h1>
          <span className="spacer" />
          {!empty && (
            <button className="btn danger" onClick={async () => {
              if (await s.confirm({ title: t("Empty the Trash?"), message: t("Everything in the Trash is deleted permanently."), danger: true, confirmLabel: t("Empty Trash") }))
                api.pages.emptyTrash().then(after).catch(s.fail);
            }}>{t("Empty Trash")}</button>
          )}
        </div>
        <p className="muted">{t("Deleted pages and tasks stay here until you delete them forever.")}</p>
        {empty ? (
          <EmptyState icon="🗑️" title={t("The Trash is empty")} />
        ) : (
          <>
            {data!.pages.map((p) => (
              <div key={p.id} className="list-row">
                <span>{p.icon || kindIcon(p.kind, p.canvasKind)}</span>
                <span className="t">{p.title || t("Untitled")}</span>
                <span className="small-text faint">{p.deletedAt && relativeTime(p.deletedAt, locale)}</span>
                <button className="btn small" onClick={() => api.pages.restore(p.id).then(after).catch(s.fail)}>{t("Restore")}</button>
                <button className="btn small danger" onClick={() => forever(p.title, () => api.pages.deleteForever(p.id))}>{t("Delete forever")}</button>
              </div>
            ))}
            {data!.tasks.map((x) => (
              <div key={x.id} className="list-row">
                <span>✅</span>
                <span className="t">{x.title}</span>
                <span className="small-text faint">{x.deletedAt && relativeTime(x.deletedAt, locale)}</span>
                <button className="btn small" onClick={() => api.tasks.restore(x.id).then(after).catch(s.fail)}>{t("Restore")}</button>
                <button className="btn small danger" onClick={() => forever(x.title, () => api.tasks.deleteForever(x.id))}>{t("Delete forever")}</button>
              </div>
            ))}
          </>
        )}
      </div>
    </div>
  );
}

export function ArchiveView() {
  const s = useStore();
  const { t, locale } = useI18n();
  const [pages, setPages] = useState<PageMeta[] | null>(null);
  const load = () => api.pages.archived().then(setPages).catch(s.fail);
  useEffect(() => {
    load();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);
  return (
    <div className="content">
      <div className="page">
        <h1 style={{ margin: "16px 0 4px", fontSize: 30 }}>🗄️ {t("Archive")}</h1>
        <p className="muted">{t("Archived pages are hidden from the sidebar but still searchable and linkable.")}</p>
        {!pages?.length ? (
          <EmptyState icon="🗄️" title={t("Nothing archived")} />
        ) : (
          pages.map((p) => (
            <div key={p.id} className="list-row" onClick={() => s.navigate({ name: "page", id: p.id })} role="link">
              <span>{p.icon || kindIcon(p.kind, p.canvasKind)}</span>
              <span className="t">{p.title || t("Untitled")}</span>
              <span className="small-text faint">{p.archivedAt && relativeTime(p.archivedAt, locale)}</span>
              <button className="btn small" onClick={(e) => (e.stopPropagation(), api.pages.unarchive(p.id).then(() => (load(), s.refreshTree())).catch(s.fail))}>{t("Unarchive")}</button>
            </div>
          ))
        )}
      </div>
    </div>
  );
}

export function SectionView({ id }: { id: string }) {
  const s = useStore();
  const { t, locale } = useI18n();
  const section = s.sections.find((x) => x.id === id);
  if (!section) return <div className="content"><EmptyState title={t("Section not found")} /></div>;
  const pages = s.tree.filter((p) => p.sectionId === id && !p.parentId).sort((a, b) => a.position - b.position);
  const kind = section.kind === "projects" ? "project" : section.kind === "databases" ? "database" : "page";
  return (
    <div className="content">
      <div className="page">
        <div className="row" style={{ marginTop: 16 }}>
          <h1 style={{ margin: 0, fontSize: 30 }}>{section.icon} {section.name}</h1>
          <span className="spacer" />
          <button className="btn primary" onClick={() => s.createPage({ kind, sectionId: id })}>＋ {t(kind === "project" ? "New project" : kind === "database" ? "New database" : "New page")}</button>
        </div>
        {pages.length === 0 ? (
          <EmptyState icon={section.icon ?? "📁"} title={t("Nothing here yet")} />
        ) : (
          <div className="home-grid" style={{ marginTop: 18 }}>
            {pages.map((p) => (
              <div key={p.id} className="tile" onClick={() => s.navigate({ name: "page", id: p.id })} role="link" tabIndex={0}>
                <span className="tile-icon">{p.icon || kindIcon(p.kind, p.canvasKind)}</span>
                <span className="tile-title">{p.title || t("Untitled")}</span>
                <span className="small-text faint">{relativeTime(p.updatedAt, locale)}</span>
              </div>
            ))}
          </div>
        )}
      </div>
    </div>
  );
}
