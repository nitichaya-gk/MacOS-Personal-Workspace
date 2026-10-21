import { useEffect, useMemo, useRef, useState } from "react";
import { useStore } from "../../app/providers/store";
import { useI18n } from "../../app/providers/i18n";
import { api, errorMessage } from "../../services/desktop-bridge/api";
import type { Backlink, PageDetail, PageMeta, Task } from "../../shared/types";
import { debounce, formatDateTime } from "../../shared/utils";
import { Icon, kindIcon } from "../../components/ui/Icon";
import { EmojiPicker } from "../../components/ui/Modal";
import { Popover, useAnchor } from "../../components/ui/Popover";
import { BlockEditor } from "../block-editor/Editor";
import { DatabaseView } from "../databases/DatabaseView";
import { RecordProperties } from "../databases/RecordProperties";
import { CanvasView } from "../diagrams/CanvasView";
import { ProjectView } from "../projects/ProjectView";
import { TaskRow } from "../tasks/TaskRow";
import { CommentsPanel } from "./history";

export function PageView({ id, viewId }: { id: string; viewId?: string }) {
  const s = useStore();
  const { t } = useI18n();
  const [detail, setDetail] = useState<PageDetail | null>(null);
  const [error, setError] = useState<string | null>(null);

  const load = () =>
    api.pages
      .open(id)
      .then((d) => (setDetail(d), setError(null)))
      .catch((e) => setError(errorMessage(e)));
  useEffect(() => {
    load();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [id]);

  if (error)
    return (
      <div className="content">
        <div className="error-box" role="alert">
          <strong>{t("This page could not be opened.")}</strong>
          <div className="mono" style={{ margin: "6px 0 10px" }}>{error}</div>
          <button className="btn" onClick={() => s.navigate({ name: "home" })}>{t("Go to Home")}</button>
        </div>
      </div>
    );
  if (!detail) return <div className="content"><div className="loading">{t("Loading…")}</div></div>;

  const meta = detail.meta;
  if (meta.kind === "canvas") return <div className="content"><CanvasView page={meta} focusElement={viewId?.startsWith("el:") ? viewId.slice(3) : undefined} /></div>;

  const section = s.sections.find((x) => x.id === meta.sectionId);
  const banners = (
    <>
      {meta.deletedAt && (
        <div className="banner warn">
          <span>🗑️ {t("This page is in the Trash.")}</span>
          <span className="spacer" />
          <button className="btn small" onClick={() => api.pages.restore(meta.id).then(() => (load(), s.refreshTree())).catch(s.fail)}>{t("Restore")}</button>
        </div>
      )}
      {meta.archivedAt && !meta.deletedAt && (
        <div className="banner">
          <span>🗄️ {t("This page is archived.")}</span>
          <span className="spacer" />
          <button className="btn small" onClick={() => api.pages.unarchive(meta.id).then(() => (load(), s.refreshTree())).catch(s.fail)}>{t("Unarchive")}</button>
        </div>
      )}
      {section?.kind === "templates" && !meta.deletedAt && (
        <div className="banner">
          <span>🧩 {t("You are editing a template. Pages created from it get a fresh copy.")}</span>
          <span className="spacer" />
          <button
            className="btn small primary"
            onClick={async () => {
              const p = await api.templates.instantiate(meta.id).catch((e) => (s.fail(e), null));
              if (p) (await s.refreshTree(), s.navigate({ name: "page", id: p.id }));
            }}
          >
            {t("Use template")}
          </button>
        </div>
      )}
      {meta.isTemplate && meta.kind === "record" && (
        <div className="banner"><span>🧩 {t("You are editing a database template.")}</span></div>
      )}
    </>
  );

  if (meta.kind === "database")
    return (
      <div className="content">
        <div className="page full">
          {banners}
          <PageHeader meta={meta} onChange={(m) => setDetail({ ...detail, meta: m })} compact />
          <DatabaseView id={meta.id} viewId={viewId} />
        </div>
      </div>
    );

  if (meta.kind === "project")
    return (
      <>
        <div className="content">
          <ProjectView detail={detail} banners={banners} onMeta={(m) => setDetail({ ...detail, meta: m })} />
        </div>
        {s.inspectorOpen && <PageInspector meta={meta} />}
      </>
    );

  return (
    <>
      <div className="content">
        <div className="page">
          {banners}
          <PageHeader meta={meta} onChange={(m) => setDetail({ ...detail, meta: m })} />
          {meta.kind === "record" && <RecordProperties recordId={meta.id} />}
          <div className="editor-wrap">
            <BlockEditor page={meta} initialBlocks={detail.blocks} readOnly={!!meta.deletedAt} />
          </div>
          <Backlinks id={meta.id} />
        </div>
      </div>
      {s.inspectorOpen && <PageInspector meta={meta} />}
    </>
  );
}

export function PageHeader({ meta, onChange, compact }: { meta: PageMeta; onChange: (m: PageMeta) => void; compact?: boolean }) {
  const s = useStore();
  const { t } = useI18n();
  const [title, setTitle] = useState(meta.title);
  const picker = useAnchor();
  const ref = useRef<HTMLTextAreaElement>(null);
  const saveTitle = useMemo(
    () => debounce((value: string) => api.pages.update(meta.id, { title: value }).then((m) => (onChange(m), s.refreshTree())).catch(s.fail), 400),
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [meta.id],
  );
  useEffect(() => () => saveTitle.flush(), [saveTitle]);
  useEffect(() => {
    const el = ref.current;
    if (el) (el.style.height = "auto"), (el.style.height = `${el.scrollHeight}px`);
  }, [title]);
  useEffect(() => {
    if (!meta.title && meta.kind !== "record") ref.current?.focus();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [meta.id]);

  const setIcon = (icon: string | null) => {
    picker.close();
    api.pages.update(meta.id, { icon }).then((m) => (onChange(m), s.refreshTree())).catch(s.fail);
  };

  return (
    <div>
      {meta.icon ? (
        <button className="page-icon" onClick={picker.toggle} aria-label={t("Change icon")} style={compact ? { fontSize: 36, marginTop: 0 } : undefined}>{meta.icon}</button>
      ) : (
        <div className="page-head-actions">
          <button className="btn small ghost" onClick={picker.toggle}>☺ {t("Add icon")}</button>
        </div>
      )}
      {picker.anchor && (
        <Popover anchor={picker.anchor} onClose={picker.close}>
          <EmojiPicker onPick={setIcon} onRemove={meta.icon ? () => setIcon(null) : undefined} />
        </Popover>
      )}
      <textarea
        ref={ref}
        className="title-input"
        rows={1}
        value={title}
        placeholder={t("Untitled")}
        aria-label={t("Page title")}
        style={compact ? { fontSize: 30, marginBottom: 4 } : undefined}
        onChange={(e) => {
          setTitle(e.target.value.replace(/\n/g, ""));
          s.setSaveStatus("saving");
          saveTitle(e.target.value.replace(/\n/g, ""));
        }}
        onKeyDown={(e) => {
          if (e.key === "Enter" && !e.nativeEvent.isComposing) {
            e.preventDefault();
            saveTitle.flush();
            (document.querySelector(".bn-editor") as HTMLElement | null)?.focus();
          }
        }}
      />
    </div>
  );
}

export function Backlinks({ id }: { id: string }) {
  const s = useStore();
  const { t } = useI18n();
  const [links, setLinks] = useState<Backlink[]>([]);
  useEffect(() => {
    api.pages.backlinks(id).then(setLinks).catch(() => {});
  }, [id, s.tree]);
  if (!links.length) return null;
  return (
    <section className="backlinks no-print" aria-label={t("Backlinks")}>
      <div className="h-section" style={{ marginTop: 0 }}>↩︎ {t("{n} backlinks", { n: links.length })}</div>
      {links.map((l) => (
        <div
          key={l.sourceId}
          className="backlink"
          role="link"
          onClick={() => (l.sourceType === "task" ? s.openTask(l.sourceId) : s.navigate({ name: "page", id: l.sourceId }))}
        >
          <div>{l.icon || kindIcon(l.kind ?? l.sourceType)} {l.title || t("Untitled")}</div>
          {l.context && <div className="ctx">{l.context}</div>}
        </div>
      ))}
    </section>
  );
}

function PageInspector({ meta }: { meta: PageMeta }) {
  const s = useStore();
  const { t, locale } = useI18n();
  const [tasks, setTasks] = useState<Task[]>([]);
  const [backlinks, setBacklinks] = useState<Backlink[]>([]);
  const reload = () => {
    api.tasks.list("all", meta.kind === "project" ? { projectId: meta.id } : { pageId: meta.id }).then(setTasks).catch(() => {});
    api.pages.backlinks(meta.id).then(setBacklinks).catch(() => {});
  };
  useEffect(reload, [meta.id, s.taskDialog]);
  const section = s.sections.find((x) => x.id === meta.sectionId);
  return (
    <aside className="inspector no-print" aria-label={t("Inspector")}>
      <h4>{t("Page")}</h4>
      <div className="kv">
        <span className="k">{t("Type")}</span><span>{t(meta.kind === "record" ? "Database record" : meta.kind === "project" ? "Project" : "Page")}</span>
        {section && (<><span className="k">{t("Section")}</span><span>{section.name}</span></>)}
        <span className="k">{t("Created")}</span><span>{formatDateTime(meta.createdAt, locale)}</span>
        <span className="k">{t("Edited")}</span><span>{formatDateTime(meta.updatedAt, locale)}</span>
      </div>
      <div className="row" style={{ marginTop: 10 }}>
        <button className="btn small" onClick={() => api.pages.update(meta.id, { favorite: !meta.favorite }).then(s.refreshTree).catch(s.fail)}>
          <Icon name="star" size={13} /> {meta.favorite ? t("Unfavorite") : t("Favorite")}
        </button>
        <button className="btn small" onClick={() => navigator.clipboard.writeText(`workspace://page/${meta.id}`).then(() => s.toast(t("Link copied")))}>
          <Icon name="link" size={13} /> {t("Copy link")}
        </button>
      </div>
      <h4 className="row">
        <span>{t("Tasks")}</span><span className="spacer" />
        <button className="icon-btn small" aria-label={t("New task")} onClick={() => s.openTask(undefined, meta.kind === "project" ? { projectId: meta.id } : { sourcePageId: meta.id })}><Icon name="plus" size={13} /></button>
      </h4>
      {tasks.length ? tasks.map((x) => <TaskRow key={x.id} task={x} compact onChanged={reload} />) : <div className="faint small-text">{t("No tasks linked to this page")}</div>}
      <h4>{t("Comments")}</h4>
      <CommentsPanel pageId={meta.id} />
      <h4>{t("Backlinks")}</h4>
      {backlinks.length ? (
        backlinks.map((l) => (
          <div key={l.sourceId} className="backlink" onClick={() => (l.sourceType === "task" ? s.openTask(l.sourceId) : s.navigate({ name: "page", id: l.sourceId }))}>
            <div>{l.icon || kindIcon(l.kind ?? l.sourceType)} {l.title || t("Untitled")}</div>
          </div>
        ))
      ) : (
        <div className="faint small-text">{t("No pages link here yet. Type @ in another page to link it.")}</div>
      )}
    </aside>
  );
}
