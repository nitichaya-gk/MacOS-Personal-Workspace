import { useEffect, useMemo, useRef } from "react";
import { filterSuggestionItems, insertOrUpdateBlockForSlashMenu } from "@blocknote/core";
import { SideMenuExtension } from "@blocknote/core/extensions";
import {
  BlockColorsItem, DragHandleMenu, RemoveBlockItem, SideMenu, SideMenuController, SuggestionMenuController,
  getDefaultReactSlashMenuItems, useBlockNoteEditor, useComponentsContext, useCreateBlockNote, useExtensionState,
  type DefaultReactSuggestionItem,
} from "@blocknote/react";
import { BlockNoteView } from "@blocknote/mantine";
import { useColorScheme, useStore } from "../../app/providers/store";
import { useI18n } from "../../app/providers/i18n";
import { api } from "../../services/desktop-bridge/api";
import type { PageMeta } from "../../shared/types";
import { debounce } from "../../shared/utils";
import { kindIcon } from "../../components/ui/Icon";
import { schema, type WorkspaceEditor } from "./schema";

function plainText(content: unknown): string {
  if (!Array.isArray(content)) return "";
  return content.map((c: { text?: string; content?: unknown }) => c.text ?? plainText(c.content)).join("");
}

/** Drag-handle menu entry: turn a checklist item into a real Task (opt-in, so no duplicates are created automatically). */
function ConvertToTaskItem({ pageId }: { pageId: string }) {
  const Components = useComponentsContext()!;
  const editor = useBlockNoteEditor(schema);
  const block = useExtensionState(SideMenuExtension, { editor, selector: (s) => s?.block });
  const { openTask, fail, refreshCounts } = useStore();
  const { t } = useI18n();
  if (!block || block.type !== "checkListItem") return null;
  return (
    <Components.Generic.Menu.Item
      className="bn-menu-item"
      onClick={async () => {
        try {
          const title = plainText(block.content).trim() || t("Untitled task");
          const task = await api.tasks.create({ title, sourcePageId: pageId, status: (block.props as { checked?: boolean }).checked ? "done" : "todo" });
          editor.replaceBlocks([block], [{ type: "taskRef", props: { taskId: task.id } }]);
          refreshCounts();
          openTask(task.id);
        } catch (e) {
          fail(e);
        }
      }}
    >
      {t("Convert to task")}
    </Components.Generic.Menu.Item>
  );
}

export function BlockEditor({ page, initialBlocks, readOnly }: { page: PageMeta; initialBlocks: unknown[]; readOnly?: boolean }) {
  const s = useStore();
  const { t } = useI18n();
  const scheme = useColorScheme();
  const sRef = useRef(s);
  sRef.current = s;

  const editor = useCreateBlockNote(
    {
      schema,
      initialContent: initialBlocks.length ? (initialBlocks as never) : undefined,
      uploadFile: async (file: File) => (await api.assets.upload(file, page.id)).url,
      tables: { splitCells: true, cellBackgroundColor: true, cellTextColor: true, headers: true },
      dictionary: undefined,
    },
    [page.id],
  ) as unknown as WorkspaceEditor;

  const save = useMemo(() => {
    let retry: ReturnType<typeof setTimeout> | undefined;
    const run = debounce(async () => {
      clearTimeout(retry);
      try {
        await api.pages.saveBlocks(page.id, editor.document as unknown[]);
        sRef.current.setSaveStatus("saved");
      } catch (e) {
        // Keep the content in the editor and retry; the status bar shows that it is not saved yet.
        sRef.current.setSaveStatus("error");
        console.error(e);
        retry = setTimeout(() => run(), 3000);
      }
    }, 500);
    return run;
  }, [editor, page.id]);

  useEffect(() => {
    s.setSaveStatus("idle");
    const flush = () => save.flush();
    window.addEventListener("blur", flush);
    window.addEventListener("beforeunload", flush);
    return () => {
      window.removeEventListener("blur", flush);
      window.removeEventListener("beforeunload", flush);
      save.flush();
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [save]);

  // Markdown import: the command palette stashes the file text for the freshly created page.
  useEffect(() => {
    const key = `import:${page.id}`;
    const md = sessionStorage.getItem(key);
    if (!md) return;
    sessionStorage.removeItem(key);
    try {
      const blocks = editor.tryParseMarkdownToBlocks(md);
      editor.replaceBlocks(editor.document, blocks);
      s.toast(t("Markdown imported"), { kind: "success" });
    } catch (e) {
      s.fail(e);
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [editor, page.id]);

  const createChild = async (kind: "page" | "database" | "canvas", canvasKind?: "diagram" | "wireframe") => {
    const child = await api.pages.create({ kind, parentId: page.id, canvasKind });
    await s.refreshTree();
    return child;
  };

  const customItems = (): DefaultReactSuggestionItem[] => [
    {
      title: t("Callout"), subtext: t("Highlighted note with an emoji"), aliases: ["callout", "note", "info"], group: t("Basic blocks"),
      icon: <span>💡</span>, onItemClick: () => insertOrUpdateBlockForSlashMenu(editor, { type: "callout" }),
    },
    {
      title: t("Subpage"), subtext: t("Create a page inside this one"), aliases: ["page", "subpage", "หน้า"], group: t("Workspace"),
      icon: <span>📄</span>,
      onItemClick: async () => {
        const child = await createChild("page").catch((e) => (s.fail(e), null));
        if (child) insertOrUpdateBlockForSlashMenu(editor, { type: "pageLink", props: { pageId: child.id } });
      },
    },
    {
      title: t("Link to page"), subtext: t("Link an existing page"), aliases: ["link", "mention"], group: t("Workspace"),
      icon: <span>🔗</span>, onItemClick: () => insertOrUpdateBlockForSlashMenu(editor, { type: "pageLink" }),
    },
    {
      title: t("Database – inline"), subtext: t("New database inside this page"), aliases: ["database", "table", "db"], group: t("Workspace"),
      icon: <span>🗂️</span>,
      onItemClick: async () => {
        const db = await createChild("database").catch((e) => (s.fail(e), null));
        if (db) insertOrUpdateBlockForSlashMenu(editor, { type: "databaseView", props: { databaseId: db.id } });
      },
    },
    {
      title: t("Linked database view"), subtext: t("Show an existing database"), aliases: ["view", "linked"], group: t("Workspace"),
      icon: <span>🔗</span>, onItemClick: () => insertOrUpdateBlockForSlashMenu(editor, { type: "databaseView" }),
    },
    {
      title: t("Diagram"), subtext: t("New whiteboard embedded here"), aliases: ["diagram", "whiteboard", "canvas", "draw"], group: t("Workspace"),
      icon: <span>🧭</span>,
      onItemClick: async () => {
        const c = await createChild("canvas", "diagram").catch((e) => (s.fail(e), null));
        if (c) insertOrUpdateBlockForSlashMenu(editor, { type: "canvasEmbed", props: { canvasId: c.id } });
      },
    },
    {
      title: t("Wireframe"), subtext: t("New wireframe embedded here"), aliases: ["wireframe", "mockup", "ui"], group: t("Workspace"),
      icon: <span>📐</span>,
      onItemClick: async () => {
        const c = await createChild("canvas", "wireframe").catch((e) => (s.fail(e), null));
        if (c) insertOrUpdateBlockForSlashMenu(editor, { type: "canvasEmbed", props: { canvasId: c.id } });
      },
    },
    {
      title: t("Embed canvas"), subtext: t("Show an existing diagram or wireframe"), aliases: ["embed", "canvas"], group: t("Workspace"),
      icon: <span>🖼</span>, onItemClick: () => insertOrUpdateBlockForSlashMenu(editor, { type: "canvasEmbed" }),
    },
    {
      title: t("Task"), subtext: t("Task with due date and reminder"), aliases: ["task", "todo", "งาน"], group: t("Workspace"),
      icon: <span>✅</span>,
      onItemClick: async () => {
        const title = await s.prompt({ title: t("New task"), placeholder: t("Task name") });
        if (!title?.trim()) return;
        try {
          const task = await api.tasks.create({ title, sourcePageId: page.id, projectId: undefined });
          insertOrUpdateBlockForSlashMenu(editor, { type: "taskRef", props: { taskId: task.id } });
          s.refreshCounts();
          s.openTask(task.id);
        } catch (e) {
          s.fail(e);
        }
      },
    },
    {
      title: t("Mermaid"), subtext: t("Diagram from Mermaid text"), aliases: ["mermaid", "flowchart", "sequence"], group: t("Advanced"),
      icon: <span>🧜</span>, onItemClick: () => insertOrUpdateBlockForSlashMenu(editor, { type: "mermaid" }),
    },
  ];

  const mentionItems = (query: string): DefaultReactSuggestionItem[] => {
    const q = query.toLowerCase();
    const pages = s.tree.filter((p) => p.id !== page.id && p.title.toLowerCase().includes(q)).slice(0, 8);
    const items: DefaultReactSuggestionItem[] = pages.map((p) => ({
      title: p.title || t("Untitled"),
      icon: <span>{p.icon || kindIcon(p.kind, p.canvasKind)}</span>,
      onItemClick: () => editor.insertInlineContent([{ type: "pageMention", props: { pageId: p.id } }, " "]),
    }));
    if (query.trim())
      items.push({
        title: t("Create page “{q}”", { q: query.trim() }),
        icon: <span>➕</span>,
        onItemClick: async () => {
          const p = await api.pages.create({ kind: "page", title: query.trim(), parentId: page.id }).catch((e) => (s.fail(e), null));
          if (!p) return;
          await s.refreshTree();
          editor.insertInlineContent([{ type: "pageMention", props: { pageId: p.id } }, " "]);
        },
      });
    return items;
  };

  return (
    <BlockNoteView
      editor={editor}
      theme={scheme}
      editable={!readOnly}
      slashMenu={false}
      sideMenu={false}
      onChange={() => {
        s.setSaveStatus("saving");
        save();
      }}
    >
      <SuggestionMenuController
        triggerCharacter="/"
        getItems={async (query) => filterSuggestionItems([...getDefaultReactSlashMenuItems(editor), ...customItems()], query)}
      />
      <SuggestionMenuController triggerCharacter="@" getItems={async (query) => mentionItems(query)} />
      <SideMenuController
        sideMenu={(props) => (
          <SideMenu
            {...props}
            dragHandleMenu={() => (
              <DragHandleMenu>
                <RemoveBlockItem>{t("Delete")}</RemoveBlockItem>
                <BlockColorsItem>{t("Colors")}</BlockColorsItem>
                <ConvertToTaskItem pageId={page.id} />
              </DragHandleMenu>
            )}
          />
        )}
      />
    </BlockNoteView>
  );
}
