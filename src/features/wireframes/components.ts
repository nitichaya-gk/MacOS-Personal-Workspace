// Wireframe UI kit and diagram shapes, built as Excalidraw element skeletons (editable objects, not images).

export type Skeleton = { type: string; x: number; y: number; [k: string]: unknown };

const INK = "#1e1e1e";
const MUTED = "#868e96";
const FILL = "#f1f3f5";
const ACCENT = "#228be6";
const base = { roughness: 0, strokeWidth: 1, strokeColor: INK, fontFamily: 2 };

const rect = (x: number, y: number, w: number, h: number, extra: Record<string, unknown> = {}): Skeleton => ({
  type: "rectangle", x, y, width: w, height: h, ...base, roundness: { type: 3, value: 6 }, ...extra,
});
const text = (x: number, y: number, t: string, extra: Record<string, unknown> = {}): Skeleton => ({ type: "text", x, y, text: t, fontSize: 16, ...base, ...extra });
const label = (t: string, extra: Record<string, unknown> = {}) => ({ label: { text: t, fontSize: 16, fontFamily: 2, strokeColor: INK, ...extra } });

export interface KitItem {
  id: string;
  label: string;
  icon: string;
  build: (x: number, y: number) => Skeleton[];
}

export const FRAMES: KitItem[] = [
  { id: "desktop", label: "Desktop 1440×900", icon: "🖥️", build: (x, y) => [{ type: "frame", x, y, width: 1440, height: 900, name: "Desktop", children: [] }] },
  { id: "tablet", label: "Tablet 834×1112", icon: "📱", build: (x, y) => [{ type: "frame", x, y, width: 834, height: 1112, name: "Tablet", children: [] }] },
  { id: "mobile", label: "Mobile 390×844", icon: "📱", build: (x, y) => [{ type: "frame", x, y, width: 390, height: 844, name: "Mobile", children: [] }] },
];

export const UI_KIT: KitItem[] = [
  { id: "button", label: "Button", icon: "🔘", build: (x, y) => [rect(x, y, 120, 40, { backgroundColor: ACCENT, fillStyle: "solid", strokeColor: ACCENT, ...label("Button", { strokeColor: "#ffffff" }) })] },
  {
    id: "input", label: "Input", icon: "⌨️",
    build: (x, y) => [text(x, y, "Label", { fontSize: 14, strokeColor: MUTED }), rect(x, y + 22, 260, 40, { ...label("Placeholder", { strokeColor: MUTED, textAlign: "left" }) })],
  },
  {
    id: "textarea", label: "Textarea", icon: "📝",
    build: (x, y) => [text(x, y, "Message", { fontSize: 14, strokeColor: MUTED }), rect(x, y + 22, 320, 110, { ...label("Write something…", { strokeColor: MUTED, textAlign: "left", verticalAlign: "top" }) })],
  },
  {
    id: "checkbox", label: "Checkbox", icon: "☑️",
    build: (x, y) => [rect(x, y, 20, 20, { roundness: { type: 3, value: 3 } }), text(x + 30, y, "Remember me")],
  },
  {
    id: "radio", label: "Radio", icon: "🔘",
    build: (x, y) => [
      { type: "ellipse", x, y, width: 20, height: 20, ...base }, { type: "ellipse", x: x + 5, y: y + 5, width: 10, height: 10, ...base, backgroundColor: INK, fillStyle: "solid" }, text(x + 30, y, "Option A"),
      { type: "ellipse", x, y: y + 32, width: 20, height: 20, ...base }, text(x + 30, y + 32, "Option B"),
    ],
  },
  {
    id: "select", label: "Select", icon: "🔽",
    build: (x, y) => [rect(x, y, 240, 40, { ...label("Choose an option", { textAlign: "left" }) }), text(x + 214, y + 9, "▾", { fontSize: 18 })],
  },
  {
    id: "tabs", label: "Tabs", icon: "🗂️",
    build: (x, y) => [
      text(x, y, "Overview", { fontSize: 15 }), { type: "line", x, y: y + 28, points: [[0, 0], [72, 0]], ...base, strokeWidth: 2 },
      text(x + 100, y, "Details", { fontSize: 15, strokeColor: MUTED }), text(x + 190, y, "Settings", { fontSize: 15, strokeColor: MUTED }),
      { type: "line", x, y: y + 29, points: [[0, 0], [320, 0]], ...base, strokeColor: "#dee2e6" },
    ],
  },
  {
    id: "sidebar", label: "Sidebar", icon: "▤",
    build: (x, y) => [
      rect(x, y, 220, 520, { backgroundColor: FILL, fillStyle: "solid", roundness: null }),
      text(x + 20, y + 20, "App name", { fontSize: 18 }),
      ...["Dashboard", "Projects", "Tasks", "Settings"].map((n, i) => text(x + 20, y + 72 + i * 36, n, { fontSize: 15, strokeColor: i === 0 ? ACCENT : INK })),
    ],
  },
  {
    id: "nav", label: "Navigation bar", icon: "☰",
    build: (x, y) => [
      rect(x, y, 960, 56, { roundness: null }), text(x + 20, y + 16, "Logo", { fontSize: 18 }),
      ...["Home", "Features", "Pricing", "About"].map((n, i) => text(x + 560 + i * 90, y + 18, n, { fontSize: 15 })),
    ],
  },
  {
    id: "card", label: "Card", icon: "🃏",
    build: (x, y) => [
      rect(x, y, 280, 300), rect(x + 16, y + 16, 248, 140, { backgroundColor: FILL, fillStyle: "solid", strokeColor: "#ced4da", ...label("Image", { strokeColor: MUTED }) }),
      text(x + 16, y + 172, "Card title", { fontSize: 18 }), text(x + 16, y + 202, "Short supporting text\nfor this card.", { fontSize: 14, strokeColor: MUTED }),
      rect(x + 16, y + 250, 100, 34, { backgroundColor: ACCENT, fillStyle: "solid", strokeColor: ACCENT, ...label("Action", { strokeColor: "#ffffff", fontSize: 14 }) }),
    ],
  },
  {
    id: "table", label: "Table", icon: "▦",
    build: (x, y) => {
      const cols = ["Name", "Status", "Owner", "Date"];
      const out: Skeleton[] = [rect(x, y, 560, 40, { backgroundColor: FILL, fillStyle: "solid", roundness: null })];
      cols.forEach((c, i) => out.push(text(x + 16 + i * 140, y + 10, c, { fontSize: 14 })));
      for (let r = 1; r <= 3; r++) {
        out.push(rect(x, y + r * 40, 560, 40, { roundness: null }));
        cols.forEach((_, i) => out.push(text(x + 16 + i * 140, y + r * 40 + 10, "—", { fontSize: 14, strokeColor: MUTED })));
      }
      return out;
    },
  },
  {
    id: "modal", label: "Modal", icon: "🪟",
    build: (x, y) => [
      rect(x, y, 440, 260, { strokeWidth: 2 }), text(x + 24, y + 22, "Dialog title", { fontSize: 20 }),
      text(x + 24, y + 64, "Explain what happens and what the user\ncan do next.", { fontSize: 14, strokeColor: MUTED }),
      rect(x + 196, y + 196, 100, 40, { ...label("Cancel", { fontSize: 14 }) }),
      rect(x + 312, y + 196, 104, 40, { backgroundColor: ACCENT, fillStyle: "solid", strokeColor: ACCENT, ...label("Confirm", { strokeColor: "#ffffff", fontSize: 14 }) }),
      text(x + 408, y + 16, "×", { fontSize: 20, strokeColor: MUTED }),
    ],
  },
  { id: "heading", label: "Heading", icon: "H", build: (x, y) => [text(x, y, "Page heading", { fontSize: 32 })] },
  { id: "paragraph", label: "Paragraph", icon: "¶", build: (x, y) => [text(x, y, "Body text goes here. Keep it short\nand scannable.", { fontSize: 16, strokeColor: "#495057" })] },
  {
    id: "image", label: "Image placeholder", icon: "🖼️",
    build: (x, y) => [
      rect(x, y, 240, 160, { backgroundColor: FILL, fillStyle: "solid", strokeColor: "#ced4da", roundness: null }),
      { type: "line", x, y, points: [[0, 0], [240, 160]], ...base, strokeColor: "#ced4da" },
      { type: "line", x: x + 240, y, points: [[0, 0], [-240, 160]], ...base, strokeColor: "#ced4da" },
    ],
  },
];

export const DIAGRAM_SHAPES: KitItem[] = [
  { id: "process", label: "Process", icon: "▭", build: (x, y) => [rect(x, y, 160, 70, { ...label("Process") })] },
  { id: "decision", label: "Decision", icon: "◇", build: (x, y) => [{ type: "diamond", x, y, width: 150, height: 100, ...base, ...label("Decision?") }] },
  { id: "terminal", label: "Start / End", icon: "⬭", build: (x, y) => [{ type: "ellipse", x, y, width: 150, height: 64, ...base, ...label("Start") }] },
  { id: "database", label: "Data store", icon: "🛢", build: (x, y) => [rect(x, y, 150, 70, { roundness: { type: 3, value: 30 }, backgroundColor: "#e7f5ff", fillStyle: "solid", ...label("Database") })] },
  { id: "sticky", label: "Sticky note", icon: "🟨", build: (x, y) => [rect(x, y, 180, 160, { backgroundColor: "#ffec99", fillStyle: "solid", strokeColor: "#f08c00", roundness: null, ...label("Note", { textAlign: "left", verticalAlign: "top" }) })] },
  { id: "text", label: "Text", icon: "T", build: (x, y) => [text(x, y, "Text", { fontSize: 20 })] },
  {
    id: "flow", label: "Two connected steps", icon: "⇢",
    build: (x, y) => [
      rect(x, y, 150, 64, { id: "a", ...label("Step 1") }),
      rect(x + 260, y, 150, 64, { id: "b", ...label("Step 2") }),
      { type: "arrow", x: x + 150, y: y + 32, width: 110, height: 0, ...base, start: { id: "a" }, end: { id: "b" } },
    ],
  },
];
