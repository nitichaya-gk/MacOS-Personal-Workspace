// BlockNote schema: the default blocks plus workspace-specific blocks and the @page mention.
import { BlockNoteSchema, defaultBlockSpecs, defaultInlineContentSpecs } from "@blocknote/core";
import { createReactBlockSpec, createReactInlineContentSpec } from "@blocknote/react";
import { useEffect, useMemo, useRef, useState } from "react";
import { useStore, useColorScheme } from "../../app/providers/store";
import { useI18n } from "../../app/providers/i18n";
import { api, errorMessage } from "../../services/desktop-bridge/api";
import type { PageKind, PageMeta, Task } from "../../shared/types";
import { kindIcon } from "../../components/ui/Icon";
import { DatabaseView } from "../databases/DatabaseView";
import { CanvasPreview } from "../diagrams/CanvasPreview";
import { TaskCheck } from "../tasks/TaskRow";
import { formatDate } from "../../shared/utils";

/** Pick an existing page (optionally of one kind) — shown by link/embed blocks until a target is chosen. */
function PagePicker({ kinds, placeholder, onPick }: { kinds?: PageKind[]; placeholder: string; onPick: (p: PageMeta) => void }) {
  const { tree } = useStore();
  const { t } = useI18n();
  const [q, setQ] = useState("");
  const list = useMemo(
    () => tree.filter((p) => (!kinds || kinds.includes(p.kind)) && p.title.toLowerCase().includes(q.trim().toLowerCase())).slice(0, 8),
    [tree, kinds, q],
  );
  return (
    <div className="cb-card" contentEditable={false}>
      <input className="input" placeholder={placeholder} value={q} onChange={(e) => setQ(e.target.value)} />
      <div style={{ marginTop: 4 }}>
        {list.map((p) => (
          <button key={p.id} className="menu-item" onClick={() => onPick(p)}>
            <span>{p.icon || kindIcon(p.kind, p.canvasKind)}</span> {p.title || t("Untitled")}
          </button>
        ))}
        {!list.length && <div className="muted small-text" style={{ padding: 6 }}>{t("No matching pages")}</div>}
      </div>
    </div>
  );
}

function useMeta(id: string) {
  const { pageById, tree } = useStore();
  const [meta, setMeta] = useState<PageMeta | null | undefined>(() => pageById(id));
  useEffect(() => {
    if (!id) return;
    const m = pageById(id);
    if (m) setMeta(m);
    else api.pages.meta(id).then((x) => setMeta(x.deletedAt ? null : x)).catch(() => setMeta(null));
  }, [id, tree, pageById]);
  return meta;
}

export const Callout = createReactBlockSpec(
  { type: "callout", propSchema: { emoji: { default: "💡" } }, content: "inline" },
  {
    render: ({ block, editor, contentRef }) => (
      <div className="cb-callout">
        <button
          className="emoji"
          contentEditable={false}
          aria-label="Change emoji"
          onClick={() => {
            const order = ["💡", "⚠️", "✅", "❗️", "📌", "🔥", "ℹ️"];
            const next = order[(order.indexOf(block.props.emoji) + 1) % order.length];
            editor.updateBlock(block, { props: { emoji: next } });
          }}
        >
          {block.props.emoji}
        </button>
        <div className="inline" ref={contentRef} />
      </div>
    ),
    toExternalHTML: ({ block, contentRef }) => (
      <blockquote>
        {block.props.emoji} <span ref={contentRef} />
      </blockquote>
    ),
  },
);

function MermaidRender({ source }: { source: string }) {
  const scheme = useColorScheme();
  const [svg, setSvg] = useState("");
  const [error, setError] = useState("");
  useEffect(() => {
    let alive = true;
    if (!source.trim()) return setSvg(""), setError("");
    const timer = setTimeout(async () => {
      try {
        const mermaid = (await import("mermaid")).default;
        mermaid.initialize({ startOnLoad: false, theme: scheme === "dark" ? "dark" : "default", securityLevel: "strict", fontFamily: "var(--font)" });
        const { svg } = await mermaid.render(`m${Math.random().toString(36).slice(2)}`, source);
        if (alive) (setSvg(svg), setError(""));
      } catch (e) {
        // Invalid syntax: keep the source untouched and show the error.
        if (alive) (setError(errorMessage(e)), setSvg(""));
      }
    }, 250);
    return () => {
      alive = false;
      clearTimeout(timer);
    };
  }, [source, scheme]);
  return (
    <>
      {error && <div className="cb-error" role="alert">{error}</div>}
      {svg && <div className="render" dangerouslySetInnerHTML={{ __html: svg }} />}
    </>
  );
}

export const Mermaid = createReactBlockSpec(
  { type: "mermaid", propSchema: { source: { default: "graph TD\n  A[Start] --> B{Decision}\n  B -->|Yes| C[Do it]\n  B -->|No| D[Skip]" } }, content: "none" },
  {
    render: function MermaidBlock({ block, editor }) {
      const { t } = useI18n();
      const [editing, setEditing] = useState(false);
      const [src, setSrc] = useState(block.props.source);
      useEffect(() => setSrc(block.props.source), [block.props.source]);
      return (
        <div className="cb cb-card cb-mermaid" contentEditable={false}>
          <div className="cb-card-head">
            <span>🧜 Mermaid</span>
            <span className="spacer" />
            <button className="btn small ghost" onClick={() => setEditing((e) => !e)}>{editing ? t("Done") : t("Edit source")}</button>
          </div>
          {editing && (
            <textarea
              className="textarea"
              value={src}
              spellCheck={false}
              onChange={(e) => setSrc(e.target.value)}
              onBlur={() => editor.updateBlock(block, { props: { source: src } })}
              onKeyDown={(e) => e.stopPropagation()}
            />
          )}
          <MermaidRender source={src} />
        </div>
      );
    },
    toExternalHTML: ({ block }) => (
      <pre>
        <code className="language-mermaid">{block.props.source}</code>
      </pre>
    ),
  },
);

export const PageLink = createReactBlockSpec(
  { type: "pageLink", propSchema: { pageId: { default: "" } }, content: "none" },
  {
    render: function PageLinkBlock({ block, editor }) {
      const { navigate } = useStore();
      const { t } = useI18n();
      const meta = useMeta(block.props.pageId);
      if (!block.props.pageId)
        return <PagePicker placeholder={t("Link to page…")} onPick={(p) => editor.updateBlock(block, { props: { pageId: p.id } })} />;
      return (
        <div className="cb-pagelink" contentEditable={false} onClick={() => meta && navigate({ name: "page", id: meta.id })} role="link">
          <span>{meta ? meta.icon || kindIcon(meta.kind, meta.canvasKind) : "⚠️"}</span>
          <span>{meta === null ? t("Deleted page") : meta?.title || t("Untitled")}</span>
        </div>
      );
    },
    toExternalHTML: ({ block }) => <p><a href={`workspace://page/${block.props.pageId}`}>📄 page</a></p>,
  },
);

export const DatabaseEmbed = createReactBlockSpec(
  { type: "databaseView", propSchema: { databaseId: { default: "" }, viewId: { default: "" } }, content: "none" },
  {
    render: function DatabaseEmbedBlock({ block, editor }) {
      const { t } = useI18n();
      if (!block.props.databaseId)
        return <PagePicker kinds={["database"]} placeholder={t("Choose a database…")} onPick={(p) => editor.updateBlock(block, { props: { databaseId: p.id } })} />;
      return (
        <div className="cb" contentEditable={false} onKeyDown={(e) => e.stopPropagation()} onPaste={(e) => e.stopPropagation()}>
          <DatabaseView
            id={block.props.databaseId}
            embedded
            viewId={block.props.viewId || undefined}
            onViewChange={(viewId) => editor.updateBlock(block, { props: { viewId } })}
          />
        </div>
      );
    },
    toExternalHTML: ({ block }) => <p><a href={`workspace://page/${block.props.databaseId}`}>🗂️ database</a></p>,
  },
);

export const CanvasEmbed = createReactBlockSpec(
  { type: "canvasEmbed", propSchema: { canvasId: { default: "" } }, content: "none" },
  {
    render: function CanvasEmbedBlock({ block, editor }) {
      const { navigate } = useStore();
      const { t } = useI18n();
      const meta = useMeta(block.props.canvasId);
      if (!block.props.canvasId)
        return <PagePicker kinds={["canvas"]} placeholder={t("Choose a diagram or wireframe…")} onPick={(p) => editor.updateBlock(block, { props: { canvasId: p.id } })} />;
      return (
        <div className="cb cb-card" contentEditable={false}>
          <div className="cb-card-head">
            <span>{meta ? kindIcon("canvas", meta.canvasKind) : "⚠️"}</span>
            <span style={{ color: "var(--text)", fontWeight: 500 }}>{meta === null ? t("Deleted canvas") : meta?.title || t("Untitled")}</span>
            <span className="spacer" />
            {meta && <button className="btn small" onClick={() => navigate({ name: "page", id: meta.id })}>{t("Open editor")}</button>}
          </div>
          {meta && (
            <div className="cb-canvas-preview" onDoubleClick={() => navigate({ name: "page", id: meta.id })} title={t("Double-click to edit")}>
              <CanvasPreview id={meta.id} />
            </div>
          )}
        </div>
      );
    },
    toExternalHTML: ({ block }) => <p><a href={`workspace://page/${block.props.canvasId}`}>🖼 canvas</a></p>,
  },
);

export const TaskRef = createReactBlockSpec(
  { type: "taskRef", propSchema: { taskId: { default: "" } }, content: "none" },
  {
    render: function TaskRefBlock({ block }) {
      const { openTask, taskDialog, refreshCounts } = useStore();
      const { t, locale } = useI18n();
      const [task, setTask] = useState<Task | null | undefined>(undefined);
      const load = useRef(() => {});
      load.current = () => {
        if (block.props.taskId) api.tasks.get(block.props.taskId).then((x) => setTask(x.deletedAt ? null : x)).catch(() => setTask(null));
      };
      // Re-read when the task dialog closes so every place showing this task stays in sync.
      useEffect(() => load.current(), [block.props.taskId, taskDialog]);
      if (task === undefined) return <div className="cb-task" contentEditable={false}>…</div>;
      if (task === null) return <div className="cb-task muted" contentEditable={false}>⚠️ {t("Deleted task")}</div>;
      return (
        <div className={`cb-task ${task.status}`} contentEditable={false}>
          <TaskCheck task={task} onChange={(x) => (setTask(x), refreshCounts())} />
          <span className="title" onClick={() => openTask(task.id)} role="button">{task.title}</span>
          {task.dueDate && <span className="small-text muted">📅 {formatDate(task.dueDate, locale)}</span>}
          {task.remindAtLocal && <span className="small-text muted">⏰ {task.remindAtLocal.slice(11)}</span>}
        </div>
      );
    },
    toExternalHTML: () => <p>☐ task</p>,
  },
);

export const PageMention = createReactInlineContentSpec(
  { type: "pageMention", propSchema: { pageId: { default: "" } }, content: "none" },
  {
    render: function Mention({ inlineContent }) {
      const { navigate } = useStore();
      const { t } = useI18n();
      const meta = useMeta(inlineContent.props.pageId);
      return (
        <span className="mention" onClick={() => meta && navigate({ name: "page", id: meta.id })} role="link">
          {meta ? meta.icon || kindIcon(meta.kind, meta.canvasKind) : "⚠️"} {meta === null ? t("Deleted page") : meta?.title || t("Untitled")}
        </span>
      );
    },
    toExternalHTML: ({ inlineContent }) => <a href={`workspace://page/${inlineContent.props.pageId}`}>@page</a>,
  },
);

export const schema = BlockNoteSchema.create({
  blockSpecs: {
    ...defaultBlockSpecs,
    callout: Callout(),
    mermaid: Mermaid(),
    pageLink: PageLink(),
    databaseView: DatabaseEmbed(),
    canvasEmbed: CanvasEmbed(),
    taskRef: TaskRef(),
  },
  inlineContentSpecs: { ...defaultInlineContentSpecs, pageMention: PageMention },
});

export type WorkspaceEditor = typeof schema.BlockNoteEditor;
