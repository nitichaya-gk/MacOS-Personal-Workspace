import { useMemo, useRef, useState } from "react";
import { addDays, clsx } from "../../shared/utils";
import { useI18n } from "../../app/providers/i18n";

export interface GanttItem {
  id: string;
  title: string;
  start: string | null; // YYYY-MM-DD
  end: string | null;
  done?: boolean;
  milestone?: boolean;
  group?: string;
  icon?: string;
  dependsOn?: string[];
}

export type GanttScale = "week" | "month" | "quarter";
const DAY_PX: Record<GanttScale, number> = { week: 40, month: 16, quarter: 6 };
/** A weekly tick this close to a month label would overlap it. */
const nearMonthStart = (dt: Date) => {
  const d = dt.getUTCDate();
  const daysInMonth = new Date(Date.UTC(dt.getUTCFullYear(), dt.getUTCMonth() + 1, 0)).getUTCDate();
  return d <= 3 || daysInMonth - d < 3;
};
const dayIndex = (from: string, d: string) => Math.round((Date.parse(`${d}T00:00:00Z`) - Date.parse(`${from}T00:00:00Z`)) / 86400000);

/**
 * Horizontal timeline. Drag a bar to move it, drag its right edge to change the end date.
 * Items without dates are listed below so nothing silently disappears.
 */
export function Gantt({ items, today, scale, onChange, onOpen, emptyHint }: {
  items: GanttItem[];
  today: string;
  scale: GanttScale;
  onChange?: (id: string, start: string, end: string) => void;
  onOpen?: (id: string) => void;
  emptyHint?: string;
}) {
  const { t, locale } = useI18n();
  const px = DAY_PX[scale];
  const dated = items.filter((i) => i.start);
  const undated = items.filter((i) => !i.start);
  const [drag, setDrag] = useState<{ id: string; mode: "move" | "resize"; x0: number; delta: number } | null>(null);
  const moved = useRef(false);

  const { from, days } = useMemo(() => {
    const starts = dated.map((i) => i.start!).concat(today);
    const ends = dated.map((i) => i.end ?? i.start!).concat(today);
    const min = starts.sort()[0];
    const max = ends.sort()[ends.length - 1];
    const f = addDays(min, -(scale === "quarter" ? 14 : 7));
    return { from: f, days: Math.max(dayIndex(f, max) + (scale === "quarter" ? 45 : 21), scale === "week" ? 28 : 60) };
  }, [dated, today, scale]);

  const width = days * px;
  const ticks: { x: number; label: string; strong: boolean }[] = [];
  for (let i = 0; i < days; i++) {
    const d = addDays(from, i);
    const dt = new Date(`${d}T00:00:00Z`);
    const first = dt.getUTCDate() === 1;
    if (scale === "week") ticks.push({ x: i * px, label: `${dt.getUTCDate()}${first ? " " + dt.toLocaleDateString(locale, { month: "short", timeZone: "UTC" }) : ""}`, strong: first });
    else if (scale === "month" ? first || (dt.getUTCDay() === 1 && !nearMonthStart(dt)) : first) {
      ticks.push({ x: i * px, label: first ? dt.toLocaleDateString(locale, { month: "short", year: "2-digit", timeZone: "UTC" }) : String(dt.getUTCDate()), strong: first });
    }
  }
  const pos = (it: GanttItem) => {
    let s = dayIndex(from, it.start!);
    let e = dayIndex(from, it.end ?? it.start!);
    if (drag?.id === it.id) {
      const dd = Math.round(drag.delta / px);
      if (drag.mode === "move") (s += dd), (e += dd);
      else e = Math.max(s, e + dd);
    }
    return { left: s * px, width: Math.max(px, (e - s + 1) * px), s, e };
  };
  const rowIndex = new Map(dated.map((it, i) => [it.id, i]));

  const onDown = (e: React.PointerEvent, it: GanttItem, mode: "move" | "resize") => {
    if (!onChange) return;
    e.stopPropagation();
    (e.target as HTMLElement).setPointerCapture(e.pointerId);
    moved.current = false;
    setDrag({ id: it.id, mode, x0: e.clientX, delta: 0 });
  };
  const onMove = (e: React.PointerEvent) => {
    if (!drag) return;
    if (Math.abs(e.clientX - drag.x0) > 3) moved.current = true;
    setDrag({ ...drag, delta: e.clientX - drag.x0 });
  };
  const onUp = (it: GanttItem) => {
    if (!drag) return;
    const p = pos(it);
    setDrag(null);
    if (moved.current && onChange) onChange(it.id, addDays(from, p.s), addDays(from, p.e));
  };

  return (
    <div>
      <div className="gantt" role="grid" aria-label={t("Timeline")}>
        <div className="gantt-head" style={{ width: width + 220 }}>
          <div className="gantt-label" style={{ height: 30, fontWeight: 600 }}>{t("Name")}</div>
          <div className="gantt-track" style={{ width, height: 30 }}>
            {ticks.map((tk, i) => <div key={i} className="gantt-tick" style={{ left: tk.x, fontWeight: tk.strong ? 600 : 400 }}>{tk.label}</div>)}
          </div>
        </div>
        <div className="gantt-grid" style={{ width: width + 220 }}>
          <div className="gantt-today" style={{ left: 220 + dayIndex(from, today) * px + px / 2 }} title={t("Today")} />
          {dated.map((it, i) => {
            const p = pos(it);
            const showGroup = it.group !== undefined && (i === 0 || dated[i - 1].group !== it.group);
            return (
              <div key={it.id}>
                {showGroup && <div className="gantt-row" style={{ height: 26 }}><div className="gantt-label small-text muted" style={{ fontWeight: 600 }}>{it.group || "—"}</div><div className="gantt-track" style={{ width }} /></div>}
                <div className="gantt-row" role="row">
                  <div className="gantt-label" role="rowheader" onClick={() => onOpen?.(it.id)} style={{ cursor: "default" }} title={it.title}>{it.icon} {it.title}</div>
                  <div className="gantt-track" style={{ width }} onPointerMove={onMove}>
                    <div
                      className={clsx("gantt-bar", it.done && "done", it.milestone && "milestone")}
                      style={{ left: p.left, width: p.width }}
                      title={`${it.title}: ${addDays(from, p.s)}${p.e !== p.s ? ` → ${addDays(from, p.e)}` : ""}`}
                      onPointerDown={(e) => onDown(e, it, "move")}
                      onPointerUp={() => onUp(it)}
                      onDoubleClick={() => onOpen?.(it.id)}
                      role="gridcell"
                      aria-label={`${it.title} ${addDays(from, p.s)}`}
                    >
                      {/* Too narrow to read: the row label and tooltip already carry the title. */}
                      {!it.milestone && p.width >= 48 && it.title}
                      {!it.milestone && onChange && <span className="handle" onPointerDown={(e) => onDown(e, it, "resize")} />}
                    </div>
                    {(it.dependsOn ?? []).map((dep) => {
                      const di = rowIndex.get(dep);
                      if (di === undefined) return null;
                      const depEnd = pos(dated[di]);
                      const x1 = depEnd.left + depEnd.width;
                      const dy = (di - i) * 34;
                      return (
                        <svg key={dep} className="gantt-dep" style={{ left: 0, top: 0, overflow: "visible" }} width={1} height={1} aria-hidden>
                          <path d={`M ${x1} ${17 + dy} H ${x1 + 6} V 17 H ${p.left}`} fill="none" stroke="var(--text-faint)" strokeWidth={1.5} markerEnd="" />
                        </svg>
                      );
                    })}
                  </div>
                </div>
              </div>
            );
          })}
          {!dated.length && <div className="muted small-text" style={{ padding: 16 }}>{emptyHint ?? t("Nothing has dates yet.")}</div>}
        </div>
      </div>
      {undated.length > 0 && (
        <div className="small-text muted" style={{ marginTop: 8 }}>
          {t("Without dates")}: {undated.map((u) => <button key={u.id} className="crumb" onClick={() => onOpen?.(u.id)}>{u.title || t("Untitled")}</button>)}
        </div>
      )}
    </div>
  );
}
