import { createContext, useCallback, useContext, useEffect, useMemo, useRef, useState, type ReactNode } from "react";
import { api, errorMessage } from "../../services/desktop-bridge/api";
import type { CanvasKind, PageKind, PageMeta, Route, Section, Settings, TaskCounts, WorkspaceInfo } from "../../shared/types";

export type SaveStatus = "idle" | "saving" | "saved" | "error";

export interface Toast {
  id: number;
  message: string;
  kind: "info" | "error" | "success";
  action?: { label: string; run: () => void };
}

export interface DialogRequest {
  kind: "confirm" | "prompt";
  title: string;
  message?: string;
  confirmLabel?: string;
  danger?: boolean;
  defaultValue?: string;
  placeholder?: string;
  resolve: (value: string | boolean | null) => void;
}

interface Store {
  ready: boolean;
  fatal: string | null;
  info: WorkspaceInfo | null;
  sections: Section[];
  tree: PageMeta[];
  settings: Settings | null;
  counts: TaskCounts | null;
  route: Route;
  canBack: boolean;
  canForward: boolean;
  navigate: (r: Route, opts?: { replace?: boolean }) => void;
  back: () => void;
  forward: () => void;
  refreshTree: () => Promise<void>;
  refreshCounts: () => Promise<void>;
  refreshAll: () => Promise<void>;
  setSettings: (s: Settings) => void;
  saveStatus: SaveStatus;
  setSaveStatus: (s: SaveStatus) => void;
  toasts: Toast[];
  toast: (message: string, opts?: { kind?: Toast["kind"]; action?: Toast["action"] }) => void;
  dismissToast: (id: number) => void;
  fail: (e: unknown) => void;
  dialog: DialogRequest | null;
  confirm: (o: { title: string; message?: string; confirmLabel?: string; danger?: boolean }) => Promise<boolean>;
  prompt: (o: { title: string; message?: string; defaultValue?: string; placeholder?: string; confirmLabel?: string }) => Promise<string | null>;
  closeDialog: (v: string | boolean | null) => void;
  createPage: (o: { kind: PageKind; parentId?: string | null; sectionId?: string | null; title?: string; canvasKind?: CanvasKind; open?: boolean }) => Promise<PageMeta | null>;
  sidebarOpen: boolean;
  toggleSidebar: () => void;
  inspectorOpen: boolean;
  toggleInspector: () => void;
  paletteOpen: boolean;
  setPaletteOpen: (o: boolean) => void;
  /** Page shown in the right-hand split pane, if any. */
  splitPageId: string | null;
  openSplit: (id: string | null) => void;
  taskDialog: { id?: string; defaults?: Record<string, unknown> } | null;
  openTask: (id?: string, defaults?: Record<string, unknown>) => void;
  closeTask: () => void;
  pageById: (id: string | null | undefined) => PageMeta | undefined;
}

const StoreContext = createContext<Store | null>(null);

/** `#/page/<id>`, `#/tasks`, `#/settings/mcp` … opens that screen first (handy for links and testing). */
function routeFromHash(): Route {
  const [name, arg, extra] = location.hash.replace(/^#\/?/, "").split("/");
  switch (name) {
    case "page":
      return arg ? { name, id: arg, tab: extra } : { name: "home" };
    case "section":
      return arg ? { name, id: arg } : { name: "home" };
    case "tasks":
      return { name, view: arg };
    case "settings":
      return { name, tab: arg };
    case "search":
      return { name, q: arg ? decodeURIComponent(arg) : undefined };
    case "trash":
    case "archive":
    case "templates":
    case "habits":
    case "finance":
    case "automations":
    case "timetable":
    case "learning":
      return { name };
    case "form":
      return arg ? { name, id: arg } : { name: "home" };
    case "goals":
      return { name, domain: arg };
    case "goal":
      return arg ? { name, id: arg, tab: extra } : { name: "goals" };
    case "skills":
      return { name, id: arg };
    case "review":
      return { name, period: arg };
    default:
      return { name: "home" };
  }
}

export function StoreProvider({ children }: { children: ReactNode }) {
  const [ready, setReady] = useState(false);
  const [fatal, setFatal] = useState<string | null>(null);
  const [info, setInfo] = useState<WorkspaceInfo | null>(null);
  const [sections, setSections] = useState<Section[]>([]);
  const [tree, setTree] = useState<PageMeta[]>([]);
  const [settings, setSettings] = useState<Settings | null>(null);
  const [counts, setCounts] = useState<TaskCounts | null>(null);
  const [history, setHistory] = useState<{ stack: Route[]; index: number }>(() => ({ stack: [routeFromHash()], index: 0 }));
  const [saveStatus, setSaveStatus] = useState<SaveStatus>("idle");
  const [toasts, setToasts] = useState<Toast[]>([]);
  const [dialog, setDialog] = useState<DialogRequest | null>(null);
  const [sidebarOpen, setSidebarOpen] = useState(true);
  const [inspectorOpen, setInspectorOpen] = useState(() => {
    try {
      return localStorage.getItem("inspector") !== "0";
    } catch {
      return true;
    }
  });
  const [paletteOpen, setPaletteOpen] = useState(false);
  const [splitPageId, openSplit] = useState<string | null>(null);
  const [taskDialog, setTaskDialog] = useState<Store["taskDialog"]>(null);
  const toastId = useRef(0);

  const toast = useCallback<Store["toast"]>((message, opts) => {
    const id = ++toastId.current;
    setToasts((t) => [...t.slice(-3), { id, message, kind: opts?.kind ?? "info", action: opts?.action }]);
    setTimeout(() => setToasts((t) => t.filter((x) => x.id !== id)), opts?.action ? 8000 : 4500);
  }, []);
  const fail = useCallback((e: unknown) => toast(errorMessage(e), { kind: "error" }), [toast]);

  const refreshTree = useCallback(async () => {
    const [s, t] = await Promise.all([api.sections.list(), api.pages.tree()]);
    setSections(s);
    setTree(t);
  }, []);
  const refreshCounts = useCallback(async () => {
    setCounts(await api.tasks.counts());
  }, []);
  const refreshAll = useCallback(async () => {
    const [i, st] = await Promise.all([api.workspace.info(), api.settings.get()]);
    setInfo(i);
    setSettings(st);
    await Promise.all([refreshTree(), refreshCounts()]);
  }, [refreshTree, refreshCounts]);

  useEffect(() => {
    refreshAll()
      .then(() => setReady(true))
      .catch((e) => setFatal(errorMessage(e)));
    // "Today" changes at midnight and when the Mac wakes up.
    const timer = setInterval(() => refreshCounts().catch(() => {}), 60_000);
    return () => clearInterval(timer);
  }, [refreshAll, refreshCounts]);

  const navigate = useCallback((r: Route, opts?: { replace?: boolean }) => {
    setHistory((h) => {
      const cur = h.stack[h.index];
      if (JSON.stringify(cur) === JSON.stringify(r)) return h;
      if (opts?.replace) {
        const stack = [...h.stack];
        stack[h.index] = r;
        return { stack, index: h.index };
      }
      const stack = [...h.stack.slice(0, h.index + 1), r].slice(-100);
      return { stack, index: stack.length - 1 };
    });
  }, []);
  const back = useCallback(() => setHistory((h) => ({ ...h, index: Math.max(0, h.index - 1) })), []);
  const forward = useCallback(() => setHistory((h) => ({ ...h, index: Math.min(h.stack.length - 1, h.index + 1) })), []);

  const confirm = useCallback<Store["confirm"]>(
    (o) => new Promise((resolve) => setDialog({ kind: "confirm", ...o, resolve: (v) => resolve(v === true) })),
    [],
  );
  const prompt = useCallback<Store["prompt"]>(
    (o) => new Promise((resolve) => setDialog({ kind: "prompt", ...o, resolve: (v) => resolve(typeof v === "string" ? v : null) })),
    [],
  );
  const closeDialog = useCallback((v: string | boolean | null) => {
    setDialog((d) => {
      d?.resolve(v);
      return null;
    });
  }, []);

  const createPage = useCallback<Store["createPage"]>(
    async (o) => {
      try {
        const p = await api.pages.create({ kind: o.kind, title: o.title ?? "", parentId: o.parentId, sectionId: o.sectionId, canvasKind: o.canvasKind });
        await refreshTree();
        if (o.open !== false) navigate({ name: "page", id: p.id });
        return p;
      } catch (e) {
        fail(e);
        return null;
      }
    },
    [refreshTree, navigate, fail],
  );

  const byId = useMemo(() => new Map(tree.map((p) => [p.id, p])), [tree]);

  const value: Store = {
    ready,
    fatal,
    info,
    sections,
    tree,
    settings,
    counts,
    route: history.stack[history.index],
    canBack: history.index > 0,
    canForward: history.index < history.stack.length - 1,
    navigate,
    back,
    forward,
    refreshTree,
    refreshCounts,
    refreshAll,
    setSettings,
    saveStatus,
    setSaveStatus,
    toasts,
    toast,
    dismissToast: (id) => setToasts((t) => t.filter((x) => x.id !== id)),
    fail,
    dialog,
    confirm,
    prompt,
    closeDialog,
    createPage,
    sidebarOpen,
    toggleSidebar: () => setSidebarOpen((o) => !o),
    inspectorOpen,
    toggleInspector: () =>
      setInspectorOpen((o) => {
        try {
          localStorage.setItem("inspector", o ? "0" : "1");
        } catch {
          /* per-viewer convenience only */
        }
        return !o;
      }),
    paletteOpen,
    splitPageId,
    openSplit,
    setPaletteOpen,
    taskDialog,
    openTask: (id, defaults) => setTaskDialog({ id, defaults }),
    closeTask: () => setTaskDialog(null),
    pageById: (id) => (id ? byId.get(id) : undefined),
  };
  return <StoreContext.Provider value={value}>{children}</StoreContext.Provider>;
}

export function useStore(): Store {
  const s = useContext(StoreContext);
  if (!s) throw new Error("useStore outside StoreProvider");
  return s;
}

/** Follows the system light/dark appearance. */
export function useColorScheme(): "light" | "dark" {
  const query = "(prefers-color-scheme: dark)";
  const [dark, setDark] = useState(() => window.matchMedia(query).matches);
  useEffect(() => {
    const mq = window.matchMedia(query);
    const on = () => setDark(mq.matches);
    mq.addEventListener("change", on);
    return () => mq.removeEventListener("change", on);
  }, []);
  return dark ? "dark" : "light";
}
