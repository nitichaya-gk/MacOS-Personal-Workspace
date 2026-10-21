import { useEffect, useRef, useState } from "react";
import { useColorScheme } from "../../app/providers/store";
import { useI18n } from "../../app/providers/i18n";
import { api } from "../../services/desktop-bridge/api";

/** Static SVG preview of a canvas, used by the canvas embed block and project lists. */
export function CanvasPreview({ id }: { id: string }) {
  const ref = useRef<HTMLDivElement>(null);
  const scheme = useColorScheme();
  const { t } = useI18n();
  const [state, setState] = useState<"loading" | "empty" | "ok" | "error">("loading");
  useEffect(() => {
    let alive = true;
    (async () => {
      try {
        const [c, ex] = await Promise.all([api.canvases.load(id), import("@excalidraw/excalidraw")]);
        if (!c.elements.length) return alive && setState("empty");
        const svg = await ex.exportToSvg({
          elements: c.elements as never,
          appState: { exportBackground: false, exportWithDarkMode: scheme === "dark", viewBackgroundColor: "transparent" } as never,
          files: Object.fromEntries(c.files.map((f) => [f.id, f])) as never,
          exportPadding: 16,
        });
        if (!alive || !ref.current) return;
        ref.current.replaceChildren(svg);
        setState("ok");
      } catch {
        if (alive) setState("error");
      }
    })();
    return () => {
      alive = false;
    };
  }, [id, scheme]);
  return (
    <>
      {state === "loading" && <span className="faint small-text">{t("Loading…")}</span>}
      {state === "empty" && <span className="faint small-text">{t("Empty canvas — open the editor to draw")}</span>}
      {state === "error" && <span className="faint small-text">{t("Preview unavailable")}</span>}
      <div ref={ref} style={{ display: state === "ok" ? "contents" : "none" }} />
    </>
  );
}
