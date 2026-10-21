// The only module that talks to the Rust side. Features never reach the database directly.
import { invoke } from "@tauri-apps/api/core";
import type {
  AssetInfo, Backlink, BackupInfo, CanvasKind, DatabaseInfo, DbRecord, PageDetail, PageKind, PageMeta, Property,
  PropertyType, QueryResult, SearchHit, Section, Settings, Task, TaskCounts, View, ViewType, WorkspaceInfo,
} from "../../shared/types";

export interface ApiError {
  code: string;
  message: string;
}

export function errorMessage(e: unknown): string {
  if (typeof e === "string") return e;
  if (e && typeof e === "object" && "message" in e) return String((e as ApiError).message);
  return String(e);
}

export const isTauri = () => typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

// Development only: `VITE_HTTP_BRIDGE=1 npm run dev` talks to `cargo run --example http_bridge` from a plain browser.
const httpBridge = import.meta.env.DEV && import.meta.env.VITE_HTTP_BRIDGE === "1" && !isTauri();

export async function rpc<T>(method: string, params: Record<string, unknown> = {}): Promise<T> {
  if (httpBridge) {
    const res = await fetch("http://127.0.0.1:1421/rpc", { method: "POST", headers: { "Content-Type": "text/plain" }, body: JSON.stringify({ method, params }) });
    const body = await res.json();
    if (!res.ok) throw body;
    return body as T;
  }
  return invoke<T>("rpc", { method, params });
}

export const api = {
  workspace: {
    info: () => rpc<WorkspaceInfo>("workspace.info"),
    rename: (name: string) => rpc<WorkspaceInfo>("workspace.rename", { name }),
    switchTo: (root: string) => rpc<WorkspaceInfo>("workspace.switch", { root }),
    create: (root: string) => rpc<WorkspaceInfo>("workspace.create", { root }),
    rebuildIndex: () => rpc<{ indexed: number }>("workspace.rebuildIndex"),
    integrity: () => rpc<Record<string, unknown>>("workspace.integrity"),
  },
  sections: {
    list: () => rpc<Section[]>("sections.list"),
    create: (name: string, icon?: string) => rpc<Section>("sections.create", { name, icon }),
    update: (id: string, patch: { name?: string; icon?: string; position?: number }) => rpc<Section[]>("sections.update", { id, ...patch }),
    remove: (id: string) => rpc("sections.delete", { id }),
  },
  pages: {
    tree: () => rpc<PageMeta[]>("pages.tree"),
    recent: (limit = 12) => rpc<PageMeta[]>("pages.recent", { limit }),
    trash: () => rpc<{ pages: PageMeta[]; tasks: Task[] }>("pages.trash"),
    archived: () => rpc<PageMeta[]>("pages.archived"),
    open: (id: string) => rpc<PageDetail>("pages.open", { id }),
    meta: (id: string) => rpc<PageMeta>("pages.meta", { id }),
    subtree: (id: string) => rpc<PageMeta[]>("pages.subtree", { id }),
    ofKind: (kind: PageKind) => rpc<PageMeta[]>("pages.ofKind", { kind }),
    create: (p: { kind: PageKind; title?: string; parentId?: string | null; sectionId?: string | null; icon?: string | null; canvasKind?: CanvasKind }) =>
      rpc<PageMeta>("pages.create", { title: "", ...p }),
    update: (id: string, patch: { title?: string; icon?: string | null; favorite?: boolean }) => rpc<PageMeta>("pages.update", { id, patch }),
    move: (id: string, parentId: string | null, sectionId: string | null, index?: number) => rpc<PageMeta>("pages.move", { id, parentId, sectionId, index }),
    duplicate: (id: string) => rpc<PageMeta>("pages.duplicate", { id }),
    archive: (id: string) => rpc("pages.archive", { id }),
    unarchive: (id: string) => rpc("pages.unarchive", { id }),
    trashPage: (id: string) => rpc("pages.trashPage", { id }),
    restore: (id: string) => rpc<PageMeta>("pages.restore", { id }),
    deleteForever: (id: string) => rpc("pages.deleteForever", { id }),
    emptyTrash: () => rpc<{ deleted: number }>("pages.emptyTrash"),
    saveBlocks: (id: string, blocks: unknown[]) => rpc<{ updatedAt: string }>("pages.saveBlocks", { id, blocks }),
    backlinks: (id: string) => rpc<Backlink[]>("pages.backlinks", { id }),
  },
  databases: {
    get: (id: string) => rpc<DatabaseInfo>("databases.get", { id }),
    query: (id: string, spec: { viewId?: string; search?: string; offset?: number; limit?: number; templates?: boolean; filter?: unknown; sorts?: unknown[] } = {}) =>
      rpc<QueryResult>("databases.query", { id, spec }),
    addProperty: (id: string, name: string, type: PropertyType) => rpc<Property>("databases.addProperty", { id, name, type }),
    updateProperty: (propertyId: string, patch: { name?: string; config?: unknown; position?: number }) => rpc<Property>("databases.updateProperty", { propertyId, patch }),
    addOption: (propertyId: string, name: string) => rpc<{ optionId: string }>("databases.addOption", { propertyId, name }),
    deleteProperty: (propertyId: string) => rpc("databases.deleteProperty", { propertyId }),
    changePropertyType: (propertyId: string, type: PropertyType, apply: boolean) =>
      rpc<{ total: number; lost: number; lostSamples: string[]; applied: boolean }>("databases.changePropertyType", { propertyId, type, apply }),
    addView: (id: string, type: ViewType, name?: string) => rpc<View>("databases.addView", { id, type, name }),
    updateView: (viewId: string, patch: { name?: string; type?: ViewType; config?: unknown }) => rpc<View>("databases.updateView", { viewId, patch }),
    deleteView: (viewId: string) => rpc("databases.deleteView", { viewId }),
    createRecord: (id: string, record: { title?: string; values?: Record<string, unknown>; templateId?: string; isTemplate?: boolean } = {}) =>
      rpc<DbRecord>("databases.createRecord", { id, record }),
    getRecord: (recordId: string) => rpc<DbRecord>("databases.getRecord", { recordId }),
    recordContext: (recordId: string) => rpc<{ database: DatabaseInfo; record: DbRecord } | null>("databases.recordContext", { recordId }),
    setValue: (recordId: string, propertyId: string, value: unknown) => rpc<DbRecord>("databases.setValue", { recordId, propertyId, value }),
    moveRecord: (recordId: string, beforeId: string | null) => rpc("databases.moveRecord", { recordId, beforeId }),
    setMcpAccess: (id: string, access: "inherit" | "deny") => rpc("databases.setMcpAccess", { id, access }),
    importCsv: (csv: string, title: string, sectionId?: string | null, parentId?: string | null) =>
      rpc<PageMeta>("databases.importCsv", { csv, title, sectionId, parentId }),
  },
  tasks: {
    list: (view: string, opts: { projectId?: string | null; pageId?: string | null; limit?: number } = {}) => rpc<Task[]>("tasks.list", { view, ...opts }),
    counts: () => rpc<TaskCounts>("tasks.counts"),
    get: (id: string) => rpc<Task>("tasks.get", { id }),
    // Patch fields mirror Task, with link lists given as ids / {requirementId, allocationPct}.
    create: (task: Record<string, unknown> & { title: string }) => rpc<Task>("tasks.create", { task }),
    update: (id: string, patch: Record<string, unknown>) => rpc<Task>("tasks.update", { id, patch }),
    remove: (id: string) => rpc("tasks.delete", { id }),
    restore: (id: string) => rpc<Task>("tasks.restore", { id }),
    deleteForever: (id: string) => rpc("tasks.deleteForever", { id }),
    missed: () => rpc<Task[]>("tasks.missed"),
    dismissMissed: () => rpc("tasks.dismissMissed"),
  },
  canvases: {
    load: (id: string) =>
      rpc<{ id: string; title: string; kind: CanvasKind; elements: unknown[]; appState: Record<string, unknown>; files: { id: string; mimeType: string; dataURL: string; created: number }[]; updatedAt: string }>(
        "canvases.load", { id }),
    save: (id: string, elements: unknown[], appState: Record<string, unknown>) => rpc<{ updatedAt: string }>("canvases.save", { id, elements, appState }),
    saveFile: (id: string, fileId: string, mimeType: string, dataURL: string) => rpc("canvases.saveFile", { id, fileId, mimeType, dataURL }),
  },
  assets: {
    listFor: (pageId: string) => rpc<AssetInfo[]>("assets.listFor", { pageId }),
    upload: async (file: File, pageId?: string | null): Promise<AssetInfo> => {
      const bytes = new Uint8Array(await file.arrayBuffer());
      return invoke<AssetInfo>("upload_asset", bytes, {
        headers: { "x-file-name": encodeURIComponent(file.name), "x-mime": file.type || "", "x-page-id": pageId ?? "" },
      });
    },
  },
  templates: {
    list: () => rpc<PageMeta[]>("templates.list"),
    instantiate: (templateId: string, target: { parentId?: string | null; sectionId?: string | null; title?: string } = {}) =>
      rpc<PageMeta>("templates.instantiate", { templateId, ...target }),
    saveAs: (pageId: string) => rpc<PageMeta>("templates.saveAs", { pageId }),
  },
  search: (query: string, filters: { types?: string[]; projectId?: string | null; modifiedAfter?: string | null; limit?: number } = {}) =>
    rpc<SearchHit[]>("search.query", { query, filters }),
  settings: {
    get: () => rpc<Settings>("settings.get"),
    set: <K extends keyof Settings>(key: K, value: Settings[K]) => rpc<Settings>("settings.set", { key, value }),
  },
  backup: {
    list: () => rpc<BackupInfo[]>("backup.list"),
    create: () => rpc<BackupInfo>("backup.create"),
    exportZip: (dest: string) => rpc<{ path: string; sizeBytes: number }>("backup.exportZip", { dest }),
    restore: (source: string, dest: string) => rpc<{ path: string; integrity: Record<string, unknown> }>("backup.restore", { source, dest }),
    exportEncrypted: (dest: string, password: string) => rpc<{ path: string; sizeBytes: number }>("backup.exportEncrypted", { dest, password }),
    restoreEncrypted: (source: string, dest: string, password: string) => rpc<{ path: string; integrity: Record<string, unknown> }>("backup.restoreEncrypted", { source, dest, password }),
    isEncrypted: (path: string) => rpc<{ encrypted: boolean }>("backup.isEncrypted", { path }),
  },
  exports: {
    pageMarkdown: (id: string, dest: string) => rpc<{ path: string; warnings: string[] }>("export.pageMarkdown", { id, dest }),
    markdownText: (id: string) => rpc<{ markdown: string; warnings: string[] }>("export.markdownText", { id }),
    database: (id: string, format: "csv" | "json", dest: string) => rpc<{ path: string; warnings: string[] }>("export.database", { id, format, dest }),
    writeText: (dest: string, text: string) => rpc<{ path: string }>("export.writeFile", { dest, text }),
    writeBase64: (dest: string, base64: string) => rpc<{ path: string }>("export.writeFile", { dest, base64 }),
    readText: (path: string) => rpc<{ text: string; name: string | null }>("import.readText", { path }),
  },
  mcp: {
    status: () =>
      rpc<{ executable: string; claudeDesktop: unknown; claudeCode: string; codex: string; audit: { client: string; tool: string; target: string | null; result: string; detail: string | null; at: string }[] }>(
        "mcp.status"),
  },
  system: {
    print: () => invoke("print_page"),
    openExternal: (target: string) => invoke("open_external", { target }),
    reveal: (path: string) => invoke("open_external", { target: path, reveal: true }),
  },
};
