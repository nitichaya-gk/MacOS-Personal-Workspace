import { useEffect, useState } from "react";
import { useStore } from "../../app/providers/store";
import { useI18n } from "../../app/providers/i18n";
import { api } from "../../services/desktop-bridge/api";
import type { PageMeta } from "../../shared/types";
import { kindIcon } from "../../components/ui/Icon";
import { EmptyState } from "../../components/ui/Modal";

export function TemplatesView() {
  const s = useStore();
  const { t } = useI18n();
  const [list, setList] = useState<PageMeta[]>([]);
  useEffect(() => {
    api.templates.list().then(setList).catch(s.fail);
  }, [s.tree, s.fail]);
  const section = s.sections.find((x) => x.kind === "templates");

  const use = async (tpl: PageMeta) => {
    const target = await s.prompt({ title: t("Name for the new page"), defaultValue: tpl.title });
    if (target === null) return;
    try {
      const p = await api.templates.instantiate(tpl.id, { title: target || tpl.title });
      await s.refreshTree();
      s.navigate({ name: "page", id: p.id });
    } catch (e) {
      s.fail(e);
    }
  };

  return (
    <div className="content">
      <div className="page">
        <div className="row" style={{ marginTop: 16 }}>
          <h1 style={{ margin: 0, fontSize: 30 }}>🧩 {t("Templates")}</h1>
          <span className="spacer" />
          {section && <button className="btn" onClick={() => s.createPage({ kind: "page", sectionId: section.id, title: t("New template") })}>＋ {t("New template")}</button>}
        </div>
        <p className="muted">{t("Using a template creates a new copy. Edit a template to change what future copies start with — existing pages are never touched. Any page can be saved as a template from its ⋯ menu.")}</p>
        {list.length === 0 ? (
          <EmptyState icon="🧩" title={t("No templates")} />
        ) : (
          <div className="home-grid">
            {list.map((tp) => (
              <div key={tp.id} className="tile" style={{ minHeight: 120 }}>
                <span className="tile-icon">{tp.icon || kindIcon(tp.kind, tp.canvasKind)}</span>
                <span className="tile-title">{tp.title || t("Untitled")}</span>
                <span className="small-text faint">{t(tp.kind === "project" ? "Project" : tp.kind === "database" ? "Database" : tp.kind === "canvas" ? "Canvas" : "Page")}</span>
                <span className="spacer" />
                <div className="row">
                  <button className="btn small primary" onClick={() => use(tp)}>{t("Use")}</button>
                  <button className="btn small" onClick={() => s.navigate({ name: "page", id: tp.id })}>{t("Edit")}</button>
                </div>
              </div>
            ))}
          </div>
        )}
      </div>
    </div>
  );
}
