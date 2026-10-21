// draw.io (.drawio / mxGraph XML) and Excalidraw (.excalidraw) import/export for canvases.
// Supported: rectangles, ellipses, diamonds, text, labelled shapes, and arrows/lines (bound or free).
// Anything else is reported back so the user knows what was simplified.
/* eslint-disable @typescript-eslint/no-explicit-any */
type El = any;

const xmlEsc = (s: string) => s.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;").replace(/"/g, "&quot;");

export function toDrawio(elements: readonly El[], title: string): { xml: string; skipped: number } {
  const live = elements.filter((e) => !e.isDeleted);
  const byId = new Map(live.map((e) => [e.id, e]));
  const labelOf = (e: El) => live.find((t) => t.type === "text" && t.containerId === e.id)?.text ?? "";
  const cells: string[] = ['<mxCell id="0"/>', '<mxCell id="1" parent="0"/>'];
  let skipped = 0;
  const color = (c: string | undefined, fallback: string) => (c && c !== "transparent" ? c : fallback);
  for (const e of live) {
    const style = `whiteSpace=wrap;html=1;strokeColor=${color(e.strokeColor, "#1e1e1e")};fillColor=${color(e.backgroundColor, "none")};${e.strokeStyle === "dashed" ? "dashed=1;" : ""}`;
    const geo = `<mxGeometry x="${Math.round(e.x)}" y="${Math.round(e.y)}" width="${Math.round(e.width)}" height="${Math.round(e.height)}" as="geometry"/>`;
    switch (e.type) {
      case "rectangle":
      case "frame":
        cells.push(`<mxCell id="${xmlEsc(e.id)}" value="${xmlEsc(e.type === "frame" ? e.name ?? "Frame" : labelOf(e))}" style="${e.type === "frame" ? "rounded=0;dashed=1;verticalAlign=top;align=left;" : e.roundness ? "rounded=1;" : "rounded=0;"}${style}" vertex="1" parent="1">${geo}</mxCell>`);
        break;
      case "ellipse":
        cells.push(`<mxCell id="${xmlEsc(e.id)}" value="${xmlEsc(labelOf(e))}" style="ellipse;${style}" vertex="1" parent="1">${geo}</mxCell>`);
        break;
      case "diamond":
        cells.push(`<mxCell id="${xmlEsc(e.id)}" value="${xmlEsc(labelOf(e))}" style="rhombus;${style}" vertex="1" parent="1">${geo}</mxCell>`);
        break;
      case "text":
        if (e.containerId && byId.has(e.containerId)) break; // already used as a shape label
        cells.push(`<mxCell id="${xmlEsc(e.id)}" value="${xmlEsc(e.text ?? "")}" style="text;html=1;align=left;verticalAlign=top;fontSize=${Math.round(e.fontSize ?? 16)};" vertex="1" parent="1">${geo}</mxCell>`);
        break;
      case "arrow":
      case "line": {
        const pts: [number, number][] = e.points ?? [[0, 0], [e.width, e.height]];
        const [sx, sy] = [e.x + pts[0][0], e.y + pts[0][1]];
        const [tx, ty] = [e.x + pts[pts.length - 1][0], e.y + pts[pts.length - 1][1]];
        const src = e.startBinding?.elementId && byId.has(e.startBinding.elementId) ? ` source="${xmlEsc(e.startBinding.elementId)}"` : "";
        const tgt = e.endBinding?.elementId && byId.has(e.endBinding.elementId) ? ` target="${xmlEsc(e.endBinding.elementId)}"` : "";
        const mid = pts.slice(1, -1).map(([x, y]) => `<mxPoint x="${Math.round(e.x + x)}" y="${Math.round(e.y + y)}"/>`).join("");
        cells.push(
          `<mxCell id="${xmlEsc(e.id)}" value="${xmlEsc(labelOf(e))}" style="endArrow=${e.type === "arrow" ? "classic" : "none"};html=1;strokeColor=${color(e.strokeColor, "#1e1e1e")};"${src}${tgt} edge="1" parent="1"><mxGeometry relative="1" as="geometry"><mxPoint x="${Math.round(sx)}" y="${Math.round(sy)}" as="sourcePoint"/><mxPoint x="${Math.round(tx)}" y="${Math.round(ty)}" as="targetPoint"/>${mid ? `<Array as="points">${mid}</Array>` : ""}</mxGeometry></mxCell>`,
        );
        break;
      }
      default:
        skipped++;
    }
  }
  const xml = `<mxfile host="PersonalWorkspace"><diagram name="${xmlEsc(title || "Page-1")}" id="pw"><mxGraphModel><root>${cells.join("")}</root></mxGraphModel></diagram></mxfile>`;
  return { xml, skipped };
}

async function inflate(b64: string): Promise<string> {
  const bytes = Uint8Array.from(atob(b64.trim()), (c) => c.charCodeAt(0));
  const stream = new Blob([bytes]).stream().pipeThrough(new DecompressionStream("deflate-raw"));
  const text = await new Response(stream).text();
  return decodeURIComponent(text);
}

const stripHtml = (s: string) => {
  const d = new DOMParser().parseFromString(`<div>${s.replace(/<br\s*\/?>/gi, "\n")}</div>`, "text/html");
  return d.body.textContent ?? "";
};

/** draw.io XML → Excalidraw element skeletons (for convertToExcalidrawElements). */
export async function fromDrawio(text: string): Promise<{ skeletons: any[]; skipped: number }> {
  const doc = new DOMParser().parseFromString(text, "application/xml");
  let model = doc.querySelector("mxGraphModel");
  if (!model) {
    const diagram = doc.querySelector("diagram");
    if (!diagram?.textContent) throw new Error("This does not look like a draw.io file.");
    const inner = new DOMParser().parseFromString(await inflate(diagram.textContent), "application/xml");
    model = inner.querySelector("mxGraphModel");
    if (!model) throw new Error("Could not read the compressed draw.io diagram.");
  }
  const cells = [...model.querySelectorAll("mxCell")];
  const geo = new Map<string, { x: number; y: number; w: number; h: number }>();
  const vertexIds = new Set<string>();
  for (const c of cells) {
    const g = c.querySelector("mxGeometry");
    if (c.getAttribute("vertex") === "1" && g) {
      vertexIds.add(c.getAttribute("id")!);
      geo.set(c.getAttribute("id")!, { x: +(g.getAttribute("x") ?? 0), y: +(g.getAttribute("y") ?? 0), w: +(g.getAttribute("width") ?? 80), h: +(g.getAttribute("height") ?? 40) });
    }
  }
  // Children of containers use coordinates relative to their parent.
  const abs = (id: string): { x: number; y: number } => {
    const c = cells.find((x) => x.getAttribute("id") === id);
    const g = geo.get(id) ?? { x: 0, y: 0, w: 0, h: 0 };
    const parent = c?.getAttribute("parent");
    if (parent && vertexIds.has(parent)) {
      const p = abs(parent);
      return { x: g.x + p.x, y: g.y + p.y };
    }
    return { x: g.x, y: g.y };
  };
  const skeletons: any[] = [];
  let skipped = 0;
  for (const c of cells) {
    const id = c.getAttribute("id")!;
    const style = c.getAttribute("style") ?? "";
    const label = stripHtml(c.getAttribute("value") ?? "").trim();
    if (c.getAttribute("vertex") === "1") {
      const g = geo.get(id);
      if (!g) continue;
      const { x, y } = abs(id);
      const base = { id: `d-${id}`, x, y, width: g.w, height: g.h, roughness: 0, strokeWidth: 1 };
      const fill = /fillColor=(#[0-9a-fA-F]{6})/.exec(style)?.[1];
      const stroke = /strokeColor=(#[0-9a-fA-F]{6})/.exec(style)?.[1];
      const colors = { ...(fill ? { backgroundColor: fill, fillStyle: "solid" } : {}), ...(stroke ? { strokeColor: stroke } : {}) };
      if (/^text;|;text;/.test(style) || style.startsWith("text")) skeletons.push({ type: "text", x, y, text: label || " ", fontSize: 16 });
      else if (style.includes("ellipse")) skeletons.push({ type: "ellipse", ...base, ...colors, ...(label ? { label: { text: label } } : {}) });
      else if (style.includes("rhombus")) skeletons.push({ type: "diamond", ...base, ...colors, ...(label ? { label: { text: label } } : {}) });
      else {
        if (/shape=|image=/.test(style) && !/shape=(rect|rectangle|process)/.test(style)) skipped++;
        skeletons.push({ type: "rectangle", ...base, ...colors, roundness: style.includes("rounded=1") ? { type: 3 } : null, ...(label ? { label: { text: label } } : {}) });
      }
    } else if (c.getAttribute("edge") === "1") {
      const g = c.querySelector("mxGeometry");
      const sp = g?.querySelector('mxPoint[as="sourcePoint"]');
      const tp = g?.querySelector('mxPoint[as="targetPoint"]');
      const src = c.getAttribute("source");
      const tgt = c.getAttribute("target");
      const centre = (vid: string | null) => {
        if (!vid || !geo.has(vid)) return null;
        const a = abs(vid);
        const gg = geo.get(vid)!;
        return { x: a.x + gg.w / 2, y: a.y + gg.h / 2 };
      };
      const p1 = centre(src) ?? (sp ? { x: +sp.getAttribute("x")!, y: +sp.getAttribute("y")! } : null);
      const p2 = centre(tgt) ?? (tp ? { x: +tp.getAttribute("x")!, y: +tp.getAttribute("y")! } : null);
      if (!p1 || !p2) {
        skipped++;
        continue;
      }
      skeletons.push({
        type: style.includes("endArrow=none") ? "line" : "arrow",
        x: p1.x, y: p1.y, width: p2.x - p1.x, height: p2.y - p1.y, points: [[0, 0], [p2.x - p1.x, p2.y - p1.y]], roughness: 0,
        ...(src && geo.has(src) && !style.includes("endArrow=none") ? { start: { id: `d-${src}` } } : {}),
        ...(tgt && geo.has(tgt) && !style.includes("endArrow=none") ? { end: { id: `d-${tgt}` } } : {}),
        ...(label ? { label: { text: label } } : {}),
      });
    }
  }
  return { skeletons, skipped };
}
