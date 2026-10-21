// Small stroke icon set (16px grid). Decorative unless a label is given.
const paths: Record<string, string> = {
  plus: "M8 3v10M3 8h10",
  chevronRight: "M6 4l4 4-4 4",
  chevronDown: "M4 6l4 4 4-4",
  chevronLeft: "M10 4L6 8l4 4",
  search: "M7 12A5 5 0 1 0 7 2a5 5 0 0 0 0 10zM14 14l-3.5-3.5",
  home: "M2.5 7.5L8 3l5.5 4.5V13a.5.5 0 0 1-.5.5H10V10H6v3.5H3a.5.5 0 0 1-.5-.5z",
  check: "M3 8.5l3 3 7-7",
  trash: "M3 4.5h10M6.5 4.5V3h3v1.5M4.5 4.5l.6 8.5h5.8l.6-8.5",
  settings: "M8 10a2 2 0 1 0 0-4 2 2 0 0 0 0 4zM8 1.5v2M8 12.5v2M1.5 8h2M12.5 8h2M3.4 3.4l1.4 1.4M11.2 11.2l1.4 1.4M3.4 12.6l1.4-1.4M11.2 4.8l1.4-1.4",
  more: "M3.5 8h.01M8 8h.01M12.5 8h.01",
  sidebar: "M2.5 3h11v10h-11zM6 3v10",
  inspector: "M2.5 3h11v10h-11zM10 3v10",
  archive: "M2 3.5h12v3H2zM3 6.5V13h10V6.5M6.5 9h3",
  star: "M8 2l1.8 3.7 4 .6-2.9 2.8.7 4L8 11.2 4.4 13.1l.7-4L2.2 6.3l4-.6z",
  template: "M2.5 2.5h11v4h-11zM2.5 9h5v4.5h-5zM10 9h3.5M10 11.5h3.5",
  calendar: "M2.5 3.5h11v10h-11zM2.5 6.5h11M5.5 2v3M10.5 2v3",
  task: "M2.5 2.5h11v11h-11zM5 8l2 2 4-4",
  page: "M4 2h5.5L12 4.5V14H4zM9.5 2v2.5H12M6 8h4M6 10.5h4",
  database: "M2.5 3h11v10h-11zM2.5 6.5h11M2.5 10h11M6.5 3v10",
  canvas: "M3.5 3.5h4v4h-4zM9.5 9.5a2 2 0 1 0 4 0 2 2 0 0 0-4 0zM7.5 5.5h2.5v4",
  wireframe: "M2 2.5h12v11H2zM2 5h12M4 7.5h4v4H4zM10 7.5h2M10 9.5h2",
  project: "M2 4.5h4.5l1.5 1.5h6v7H2z",
  link: "M7 9a3 3 0 0 0 4.2 0l2-2a3 3 0 0 0-4.2-4.2l-.8.8M9 7a3 3 0 0 0-4.2 0l-2 2a3 3 0 0 0 4.2 4.2l.8-.8",
  filter: "M2 3.5h12L9.5 9v4l-3-1.5V9z",
  sort: "M5 3v10M2.5 10.5L5 13l2.5-2.5M11 13V3M8.5 5.5L11 3l2.5 2.5",
  eye: "M1.5 8S4 3.5 8 3.5 14.5 8 14.5 8 12 12.5 8 12.5 1.5 8 1.5 8zM8 10a2 2 0 1 0 0-4 2 2 0 0 0 0 4z",
  close: "M4 4l8 8M12 4l-8 8",
  drag: "M6 4h.01M10 4h.01M6 8h.01M10 8h.01M6 12h.01M10 12h.01",
  external: "M9 2.5h4.5V7M13.5 2.5L7.5 8.5M11.5 9.5v4h-9v-9h4",
  copy: "M5.5 5.5h8v8h-8zM10.5 5.5v-3h-8v8h3",
  download: "M8 2v8.5M4.5 7L8 10.5 11.5 7M2.5 13.5h11",
  upload: "M8 10.5V2M4.5 5.5L8 2l3.5 3.5M2.5 13.5h11",
  clock: "M8 14A6 6 0 1 0 8 2a6 6 0 0 0 0 12zM8 4.5V8l2.5 1.5",
  bell: "M4 11V7a4 4 0 0 1 8 0v4l1.5 1.5h-11zM6.5 13.5a1.5 1.5 0 0 0 3 0",
  table: "M2.5 3h11v10h-11zM2.5 6.5h11M6.5 3v10",
  board: "M2.5 3h3v10h-3zM6.5 3h3v6h-3zM10.5 3h3v8h-3z",
  list: "M5.5 4h8M5.5 8h8M5.5 12h8M2.5 4h.01M2.5 8h.01M2.5 12h.01",
  restore: "M3 8a5 5 0 1 0 1.5-3.5M3 2.5v2.5h2.5",
  print: "M4 6V2.5h8V6M4 11.5H2.5V6h11v5.5H12M4.5 9.5h7v4h-7z",
  folder: "M2 4h4.5L8 5.5h6V13H2z",
  mcp: "M5 2.5v3M11 2.5v3M3.5 5.5h9v3a4.5 4.5 0 0 1-9 0zM8 13v1.5",
};

export function Icon({ name, size = 16, label, className }: { name: keyof typeof paths | string; size?: number; label?: string; className?: string }) {
  const d = paths[name] ?? paths.page;
  const filled = name === "more" || name === "drag";
  return (
    <svg
      width={size}
      height={size}
      viewBox="0 0 16 16"
      fill="none"
      stroke="currentColor"
      strokeWidth={filled ? 2.6 : 1.4}
      strokeLinecap="round"
      strokeLinejoin="round"
      className={className}
      role={label ? "img" : undefined}
      aria-label={label}
      aria-hidden={label ? undefined : true}
    >
      <path d={d} />
    </svg>
  );
}

export const kindIcon = (kind: string, canvasKind?: string | null) =>
  kind === "project" ? "📁" : kind === "database" ? "🗂️" : kind === "canvas" ? (canvasKind === "wireframe" ? "📐" : "🧭") : kind === "task" ? "✅" : "📄";
