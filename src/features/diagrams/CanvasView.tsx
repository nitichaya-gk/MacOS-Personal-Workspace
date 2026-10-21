import { lazy, Suspense, useEffect, useMemo, useRef, useState } from "react";
import { save } from "@tauri-apps/plugin-dialog";
import "@excalidraw/excalidraw/index.css";
import type * as ExcalidrawModule from "@excalidraw/excalidraw";
import { useColorScheme, useStore } from "../../app/providers/store";
import { useI18n } from "../../app/providers/i18n";
import { api, errorMessage } from "../../services/desktop-bridge/api";
import type { PageMeta } from "../../shared/types";
import { blobToBase64, debounce, fileSafeName } from "../../shared/utils";
import { kindIcon } from "../../components/ui/Icon";
import { DIAGRAM_SHAPES, FRAMES, UI_KIT, type KitItem, type Skeleton } from "../wireframes/components";

type Ex = typeof ExcalidrawModule;
// Excalidraw's element/appState types are deep; this module treats them structurally.
/* eslint-disable @typescript-eslint/no-explicit-any */
type Api = any;
type El = any;

const Excalidraw = lazy(() => import("@excalidraw/excalidraw").then((m) => ({ default: m.Excalidraw })));
let exModule: Promise<Ex> | null = null;
export const loadExcalidraw = () => (exModule ??= import("@excalidraw/excalidraw"));

const SAVED_APP_STATE = ["viewBackgroundColor", "gridModeEnabled", "gridSize", "gridStep", "objectsSnapModeEnabled", "scrollX", "scrollY", "zoom", "currentItemRoughness", "currentItemFontFamily"];

export function CanvasView({ page }: { page: PageMeta }) {
  const s = useStore();
  const { t, lang } = useI18n();
  const scheme = useColorScheme();
  const [initial, setInitial] = useState<any>(null);
  const [error, setError] = useState<string | null>(null);
  const [exApi, setExApi] = useState<Api>(null);
  const [selected, setSelected] = useState<El[]>([]);
  const kind = page.canvasKind ?? "diagram";
  const lastVersion = useRef<number>(-1);
  const savedFiles = useRef<Set<string>>(new Set());
  const sRef = useRef(s);
  sRef.current = s;

  useEffect(() => {
    api.canvases
      .load(page.id)
      .then((c) => {
        c.files.forEach((f) => savedFiles.current.add(f.id));
        const wire = c.kind === "wireframe";
        setInitial({
          elements: c.elements,
          appState: {
            currentItemRoughness: wire ? 0 : 1,
            currentItemFontFamily: wire ? 2 : 5,
            objectsSnapModeEnabled: true,
            gridModeEnabled: wire,
            ...c.appState,
            collaborators: new Map(),
          },
          files: Object.fromEntries(c.files.map((f) => [f.id, f])),
          scrollToContent: !c.appState.scrollX,
        });
      })
      .catch((e) => setError(errorMessage(e)));
  }, [page.id]);

  const persist = useMemo(
    () =>
      debounce(async (elements: readonly El[], appState: any, files: Record<string, any>) => {
        try {
          for (const [fid, f] of Object.entries(files)) {
            if (!savedFiles.current.has(fid) && elements.some((e) => e.fileId === fid && !e.isDeleted)) {
              await api.canvases.saveFile(page.id, fid, f.mimeType, f.dataURL);
              savedFiles.current.add(fid);
            }
          }
          const state = Object.fromEntries(SAVED_APP_STATE.filter((k) => k in appState).map((k) => [k, appState[k]]));
          await api.canvases.save(page.id, elements.filter((e) => !e.isDeleted), state);
          sRef.current.setSaveStatus("saved");
        } catch (e) {
          sRef.current.setSaveStatus("error");
          sRef.current.fail(e);
        }
      }, 700),
    [page.id],
  );
  useEffect(() => () => persist.flush(), [persist]);

  const onChange = async (elements: readonly El[], appState: any, files: Record<string, any>) => {
    const ex = await loadExcalidraw();
    const v = ex.getSceneVersion(elements);
    const ids = Object.keys(appState.selectedElementIds ?? {});
    setSelected((prev) => (prev.length === ids.length && prev.every((p, i) => p.id === ids[i] && p.version === elements.find((e) => e.id === ids[i])?.version) ? prev : elements.filter((e) => ids.includes(e.id) && !e.isDeleted)));
    if (lastVersion.current === -1) {
      lastVersion.current = v;
      return;
    }
    if (v === lastVersion.current) return;
    lastVersion.current = v;
    sRef.current.setSaveStatus("saving");
    persist(elements, appState, files);
  };

  const insert = async (item: KitItem) => {
    if (!exApi) return;
    const ex = await loadExcalidraw();
    const st = exApi.getAppState();
    const cx = -st.scrollX + st.width / 2 / st.zoom.value - 80;
    const cy = -st.scrollY + st.height / 2 / st.zoom.value - 40;
    const skeleton: Skeleton[] = item.build(Math.round(cx), Math.round(cy));
    const made = ex.convertToExcalidrawElements(skeleton as any);
    const group = made.length > 1 && !made.some((e: El) => e.type === "frame") ? crypto.randomUUID() : null;
    const els = group ? made.map((e: El) => ({ ...e, groupIds: [...e.groupIds, group] })) : made;
    exApi.updateScene({
      elements: [...exApi.getSceneElementsIncludingDeleted(), ...els],
      appState: { selectedElementIds: Object.fromEntries(els.map((e: El) => [e.id, true])) },
      captureUpdate: ex.CaptureUpdateAction.IMMEDIATELY,
    });
  };

  const exportAs = async (format: "png" | "svg" | "json", onlySelection: boolean) => {
    if (!exApi) return;
    const ex = await loadExcalidraw();
    const all = exApi.getSceneElements();
    const elements = onlySelection && selected.length ? all.filter((e: El) => selected.some((x) => x.id === e.id) || selected.some((x) => x.groupIds?.some((g: string) => e.groupIds?.includes(g)))) : all;
    const appState = { ...exApi.getAppState(), exportBackground: true, exportWithDarkMode: false };
    const files = exApi.getFiles();
    const dest = await save({ defaultPath: `${fileSafeName(page.title)}.${format === "json" ? "excalidraw" : format}`, filters: [{ name: format.toUpperCase(), extensions: [format === "json" ? "excalidraw" : format] }] });
    if (!dest) return;
    try {
      if (format === "png") {
        const blob = await ex.exportToBlob({ elements, appState, files, mimeType: "image/png", exportPadding: 24, getDimensions: (w: number, h: number) => ({ width: w * 2, height: h * 2, scale: 2 }) });
        await api.exports.writeBase64(dest, await blobToBase64(blob));
      } else if (format === "svg") {
        const svg = await ex.exportToSvg({ elements, appState, files, exportPadding: 24 });
        await api.exports.writeText(dest, svg.outerHTML);
      } else {
        await api.exports.writeText(dest, ex.serializeAsJSON(elements, appState, files, "local"));
      }
      s.toast(t("Exported"), { kind: "success" });
    } catch (e) {
      s.fail(e);
    }
  };

  if (error) return <div className="error-box">{t("This canvas could not be opened.")} <span className="mono">{error}</span></div>;
  if (!initial) return <div className="loading">{t("Loading…")}</div>;

  const kit = kind === "wireframe" ? [{ title: t("Frames"), items: FRAMES }, { title: t("Components"), items: UI_KIT }] : [{ title: t("Shapes"), items: DIAGRAM_SHAPES }];

  return (
    <div className="canvas-host">
      <aside className="canvas-palette" aria-label={t("Insert")}>
        <div className="row" style={{ padding: "0 4px 6px" }}>
          <strong style={{ overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>{kindIcon("canvas", kind)} {page.title || t("Untitled")}</strong>
        </div>
        {kit.map((g) => (
          <div key={g.title}>
            <h5>{g.title}</h5>
            {g.items.map((it) => (
              <button key={it.id} className="menu-item" onClick={() => insert(it)} title={t("Insert {name}", { name: t(it.label) })}>
                <span style={{ width: 18, textAlign: "center" }}>{it.icon}</span> {t(it.label)}
              </button>
            ))}
          </div>
        ))}
        <h5>{t("Export")}</h5>
        <button className="menu-item" onClick={() => exportAs("png", false)}>🖼 PNG</button>
        <button className="menu-item" onClick={() => exportAs("svg", false)}>✒️ SVG</button>
        <button className="menu-item" onClick={() => exportAs("json", false)}>{"{ }"} JSON (.excalidraw)</button>
        {selected.length > 0 && <button className="menu-item" onClick={() => exportAs("png", true)}>🖼 {t("Selection as PNG")}</button>}
        {selected.length > 0 && <button className="menu-item" onClick={() => exportAs("svg", true)}>✒️ {t("Selection as SVG")}</button>}
        <p className="small-text faint" style={{ padding: "8px 4px" }}>
          {t("Tip: right-click objects for align, distribute, layers, lock, group and duplicate. ⌘' toggles the grid.")}
        </p>
      </aside>
      <div className="excalidraw-wrap">
        <Suspense fallback={<div className="loading">{t("Loading canvas…")}</div>}>
          <Excalidraw
            initialData={initial}
            excalidrawAPI={setExApi}
            theme={scheme}
            langCode={lang === "th" ? "th-TH" : "en"}
            onChange={onChange as any}
            onLinkOpen={(el: El, ev: any) => {
              const link: string = el.link ?? "";
              if (link.startsWith("workspace://")) {
                ev.preventDefault();
                const id = link.split("/").pop();
                if (id) s.navigate({ name: "page", id });
              } else if (/^https?:/.test(link)) {
                ev.preventDefault();
                api.system.openExternal(link);
              }
            }}
            UIOptions={{ canvasActions: { loadScene: false, saveToActiveFile: false, toggleTheme: false, export: false, saveAsImage: false } }}
          />
        </Suspense>
      </div>
      {selected.length > 0 && exApi && <CanvasInspector api={exApi} selected={selected} />}
    </div>
  );
}

/** Geometry, label and workspace link of the selected object(s). Styling stays in Excalidraw's own panel. */
function CanvasInspector({ api: exApi, selected }: { api: Api; selected: El[] }) {
  const { t } = useI18n();
  const s = useStore();
  const [q, setQ] = useState("");
  const el = selected.find((e) => !e.containerId) ?? selected[0];
  const all: El[] = exApi.getSceneElementsIncludingDeleted();
  const boundText = el.boundElements?.find((b: El) => b.type === "text");
  const textEl = el.type === "text" ? el : boundText ? all.find((e) => e.id === boundText.id) : null;

  const update = async (ids: string[], patch: (e: El) => El) => {
    const ex = await loadExcalidraw();
    exApi.updateScene({
      elements: exApi.getSceneElementsIncludingDeleted().map((e: El) => (ids.includes(e.id) ? ex.newElementWith(e, patch(e)) : e)),
      captureUpdate: ex.CaptureUpdateAction.IMMEDIATELY,
    });
  };
  const num = (key: "x" | "y" | "width" | "height") => (
    <label className="field" style={{ marginBottom: 6 }}>
      <span className="field-label">{key === "x" ? "X" : key === "y" ? "Y" : key === "width" ? t("Width") : t("Height")}</span>
      <input className="input" type="number" defaultValue={Math.round(el[key])} key={`${el.id}-${key}-${Math.round(el[key])}`}
        onBlur={(e) => { const v = Number(e.target.value); if (!Number.isNaN(v) && v !== Math.round(el[key])) update([el.id], () => ({ [key]: v })); }}
        onKeyDown={(e) => e.key === "Enter" && (e.target as HTMLInputElement).blur()} />
    </label>
  );
  const pages = s.tree.filter((p) => p.title.toLowerCase().includes(q.trim().toLowerCase())).slice(0, 6);
  const linkId = typeof el.link === "string" && el.link.startsWith("workspace://") ? el.link.split("/").pop() : null;
  const linked = linkId ? s.pageById(linkId) : undefined;

  return (
    <aside className="canvas-inspector" aria-label={t("Inspector")}>
      <h4 style={{ marginTop: 0 }} className="small-text muted">{selected.length > 1 ? t("{n} objects selected", { n: selected.length }) : t(String(el.type))}</h4>
      {selected.length === 1 && (
        <>
          <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: 6 }}>{num("x")}{num("y")}{num("width")}{num("height")}</div>
          {textEl && (
            <label className="field">
              <span className="field-label">{t("Text")}</span>
              <textarea className="textarea" rows={3} defaultValue={textEl.originalText ?? textEl.text} key={textEl.id + textEl.version}
                onBlur={(e) => e.target.value !== (textEl.originalText ?? textEl.text) && update([textEl.id], () => ({ text: e.target.value, originalText: e.target.value }))} />
            </label>
          )}
        </>
      )}
      <div className="field">
        <span className="field-label">{t("Link to workspace page")}</span>
        {linked ? (
          <div className="row">
            <button className="crumb" onClick={() => s.navigate({ name: "page", id: linked.id })}>{linked.icon || kindIcon(linked.kind, linked.canvasKind)} {linked.title || t("Untitled")}</button>
            <span className="spacer" />
            <button className="btn small" onClick={() => update([el.id], () => ({ link: null }))}>{t("Remove")}</button>
          </div>
        ) : (
          <>
            <input className="input" placeholder={t("Search pages")} value={q} onChange={(e) => setQ(e.target.value)} />
            {q && pages.map((p) => (
              <button key={p.id} className="menu-item" onClick={() => (update([el.id], () => ({ link: `workspace://page/${p.id}` })), setQ(""))}>
                {p.icon || kindIcon(p.kind, p.canvasKind)} {p.title || t("Untitled")}
              </button>
            ))}
          </>
        )}
      </div>
      <div className="row wrap">
        <button className="btn small" onClick={() => update(selected.map((e) => e.id), (e) => ({ locked: !e.locked }))}>{el.locked ? "🔓 " + t("Unlock") : "🔒 " + t("Lock")}</button>
      </div>
    </aside>
  );
}
