import { useEffect, useMemo, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { useI18n } from "../../app/providers/i18n";
import { loadExcalidraw } from "./CanvasView";
/* eslint-disable @typescript-eslint/no-explicit-any */

export const FRAME_LINK = "workspace://frame/";

/**
 * Click-through prototype: shows one wireframe frame at a time; objects linked to another frame
 * (Inspector → "Link to frame") become hotspots that navigate to it.
 */
export function PrototypePlayer({ api, onClose }: { api: any; onClose: () => void }) {
  const { t } = useI18n();
  const elements: any[] = useMemo(() => api.getSceneElements().filter((e: any) => !e.isDeleted), [api]);
  const frames = useMemo(() => elements.filter((e) => e.type === "frame").sort((a, b) => a.y - b.y || a.x - b.x), [elements]);
  const [current, setCurrent] = useState<string | null>(frames[0]?.id ?? null);
  const [history, setHistory] = useState<string[]>([]);
  const [svg, setSvg] = useState<SVGSVGElement | null>(null);
  const host = useRef<HTMLDivElement>(null);
  const frame = frames.find((f) => f.id === current);

  useEffect(() => {
    if (!frame) return;
    let alive = true;
    loadExcalidraw().then(async (ex) => {
      const inFrame = elements.filter((e) => e.frameId === frame.id || e.id === frame.id);
      const out = await ex.exportToSvg({ elements: inFrame, appState: { ...api.getAppState(), exportBackground: true, viewBackgroundColor: "#ffffff", exportWithDarkMode: false }, files: api.getFiles(), exportingFrame: frame, exportPadding: 0 });
      if (alive) setSvg(out);
    });
    return () => {
      alive = false;
    };
  }, [frame, elements, api]);
  useEffect(() => {
    if (host.current && svg) {
      svg.setAttribute("width", "100%");
      svg.removeAttribute("height");
      host.current.replaceChildren(svg);
    }
  }, [svg]);
  useEffect(() => {
    const k = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
      if (e.key === "Backspace" && history.length) go(history[history.length - 1], true);
    };
    window.addEventListener("keydown", k);
    return () => window.removeEventListener("keydown", k);
  });
  const go = (id: string, back = false) => {
    if (!back && current) setHistory([...history, current]);
    if (back) setHistory(history.slice(0, -1));
    setCurrent(id);
  };

  const hotspots = frame
    ? elements.filter((e) => e.frameId === frame.id && typeof e.link === "string" && e.link.startsWith(FRAME_LINK) && frames.some((f) => f.id === e.link.slice(FRAME_LINK.length)))
    : [];

  return createPortal(
    <div className="overlay" style={{ paddingTop: 24, alignItems: "flex-start", background: "rgba(15,15,15,.75)" }} onMouseDown={(e) => e.target === e.currentTarget && onClose()}>
      <div style={{ width: "min(1100px, calc(100vw - 32px))" }}>
        <div className="row" style={{ color: "white", marginBottom: 8 }}>
          <strong>▶ {t("Prototype")}</strong>
          <select className="select" style={{ width: 220, height: 26 }} value={current ?? ""} onChange={(e) => go(e.target.value)} aria-label={t("Screen")}>
            {frames.map((f) => <option key={f.id} value={f.id}>{f.name ?? t("Frame")}</option>)}
          </select>
          <button className="btn small" disabled={!history.length} onClick={() => go(history[history.length - 1], true)}>← {t("Back")}</button>
          <span className="spacer" />
          <span className="small-text" style={{ opacity: 0.8 }}>{t("{n} hotspots on this screen", { n: hotspots.length })}</span>
          <button className="btn small" onClick={onClose}>{t("Close")}</button>
        </div>
        {!frame ? (
          <div className="card-box" style={{ background: "var(--bg)" }}>{t("Add frames (Desktop / Tablet / Mobile) to the wireframe first, then link buttons to frames in the inspector.")}</div>
        ) : (
          <div style={{ position: "relative", background: "white", borderRadius: 8, overflow: "hidden", maxHeight: "calc(100vh - 90px)", aspectRatio: `${frame.width} / ${frame.height}`, margin: "0 auto", maxWidth: `calc((100vh - 90px) * ${frame.width / frame.height})` }}>
            <div ref={host} style={{ width: "100%", height: "100%" }} />
            {hotspots.map((h) => (
              <button
                key={h.id}
                aria-label={t("Go to {name}", { name: frames.find((f) => f.id === h.link.slice(FRAME_LINK.length))?.name ?? "" })}
                onClick={() => go(h.link.slice(FRAME_LINK.length))}
                style={{
                  position: "absolute", border: "2px solid transparent", borderRadius: 4, background: "transparent", cursor: "pointer",
                  left: `${((h.x - frame.x) / frame.width) * 100}%`, top: `${((h.y - frame.y) / frame.height) * 100}%`,
                  width: `${(h.width / frame.width) * 100}%`, height: `${(h.height / frame.height) * 100}%`,
                }}
                onMouseEnter={(e) => (e.currentTarget.style.borderColor = "rgba(34,139,230,.7)")}
                onMouseLeave={(e) => (e.currentTarget.style.borderColor = "transparent")}
              />
            ))}
          </div>
        )}
      </div>
    </div>,
    document.body,
  );
}
