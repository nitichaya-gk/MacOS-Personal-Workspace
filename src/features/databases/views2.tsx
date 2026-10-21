import { useMemo, useRef, useState } from "react";
import { useStore } from "../../app/providers/store";
import { useI18n } from "../../app/providers/i18n";
import { api } from "../../services/desktop-bridge/api";
import type { DbRecord, Property } from "../../shared/types";
import { Gantt, type GanttScale } from "../../components/ui/Gantt";
import { optionOf, PropertyValue, valueOf } from "./cells";
import { visibleProps, type ViewProps } from "./views";

export function TimelineView({ db, view, records, replace, open, reload }: ViewProps) {
  const s = useStore();
  const { t } = useI18n();
  const dateProp = db.properties.find((p) => p.id === view.config.dateProperty && p.type === "date") ?? db.properties.find((p) => p.type === "date");
  const scale: GanttScale = view.config.timelineScale ?? "month";
  if (!dateProp)
    return (
      <div className="empty">
        <div>{t("Timeline views need a Date property (use an end date for a range).")}</div>
        <div className="actions"><button className="btn" onClick={() => api.databases.addProperty(db.id, "Date", "date").then(reload).catch(s.fail)}>{t("Add a Date property")}</button></div>
      </div>
    );
  const items = records.map((r) => {
    const d = valueOf(r, dateProp) as { start: string; end?: string } | null;
    return { id: r.id, title: r.title || t("Untitled"), icon: r.icon ?? undefined, start: d?.start?.slice(0, 10) ?? null, end: d?.end?.slice(0, 10) ?? null };
  });
  return (
    <div style={{ paddingTop: 6 }}>
      <Gantt
        items={items}
        today={s.counts?.date ?? new Date().toISOString().slice(0, 10)}
        scale={scale}
        onOpen={(id) => { const r = records.find((x) => x.id === id); if (r) open(r); }}
        onChange={(id, start, end) => api.databases.setValue(id, dateProp.id, start === end ? { start } : { start, end }).then(replace).catch(s.fail)}
        emptyHint={t("No records have a {name} yet.", { name: dateProp.name })}
      />
    </div>
  );
}

function coverUrl(r: DbRecord, p: Property | undefined): string | null {
  if (!p) return null;
  const files = (r.values[p.id] as { assetId: string; name: string }[] | undefined) ?? [];
  const img = files.find((f) => /\.(png|jpe?g|gif|webp|svg|heic)$/i.test(f.name) || /\.(png|jpe?g|gif|webp|svg)$/i.test(f.assetId));
  return img ? `wsasset://localhost/${img.assetId}` : null;
}

export function GalleryView({ db, view, records, create, open }: ViewProps) {
  const { t } = useI18n();
  const cover = db.properties.find((p) => p.id === view.config.coverProperty && p.type === "files");
  const shown = visibleProps(db, view).filter((p) => p.type !== "title" && p.id !== cover?.id).slice(0, 4);
  return (
    <div className="gallery">
      {records.map((r) => {
        const url = coverUrl(r, cover);
        return (
          <div key={r.id} className="card" onClick={() => open(r)} role="button" tabIndex={0} onKeyDown={(e) => e.key === "Enter" && open(r)}>
            <div className="cover">{url ? <img src={url} alt="" loading="lazy" /> : <span aria-hidden>{r.icon || "📄"}</span>}</div>
            <div className="body">
              <div className="card-title">{r.title || <span className="faint">{t("Untitled")}</span>}</div>
              <div className="card-props">{shown.map((p) => <PropertyValue key={p.id} p={p} rec={r} />)}</div>
            </div>
          </div>
        );
      })}
      <button className="card add-row" style={{ minHeight: 120, textAlign: "center" }} onClick={() => create()}>＋ {t("New")}</button>
    </div>
  );
}

interface Bucket {
  key: string;
  label: string;
  value: number;
  count: number;
}

/** Groups records and aggregates them; dates group by month. */
function buckets(records: DbRecord[], group: Property | undefined, agg: "count" | "sum" | "average", valueProp: Property | undefined, t: (s: string) => string): Bucket[] {
  const map = new Map<string, Bucket & { sum: number; n: number }>();
  const add = (key: string, label: string, r: DbRecord) => {
    const b = map.get(key) ?? { key, label, value: 0, count: 0, sum: 0, n: 0 };
    b.count += 1;
    const v = valueProp ? valueOf(r, valueProp) : null;
    if (typeof v === "number") (b.sum += v), (b.n += 1);
    map.set(key, b);
  };
  for (const r of records) {
    if (!group) add("all", t("All records"), r);
    else {
      const v = valueOf(r, group);
      if (group.type === "multi_select") {
        const ids = (v as string[] | null) ?? [];
        if (!ids.length) add("", t("No value"), r);
        ids.forEach((id) => add(id, optionOf(group, id)?.name ?? "?", r));
      } else if (group.type === "select" || group.type === "status") {
        add(String(v ?? ""), v ? optionOf(group, v)?.name ?? "?" : t("No value"), r);
      } else if (group.type === "checkbox") {
        add(v ? "1" : "0", v ? t("Checked") : t("Unchecked"), r);
      } else if (group.type === "date" || group.type === "created_time" || group.type === "last_edited_time") {
        const d = group.type === "date" ? (v as { start?: string } | null)?.start : (v as string | null);
        add(d ? d.slice(0, 7) : "", d ? d.slice(0, 7) : t("No date"), r);
      } else {
        const text = String(v ?? "").trim();
        add(text, text || t("No value"), r);
      }
    }
  }
  const out = [...map.values()].map((b) => ({ ...b, value: agg === "count" ? b.count : agg === "sum" ? b.sum : b.n ? b.sum / b.n : 0 }));
  if (group && (group.type === "select" || group.type === "status" || group.type === "multi_select")) {
    const order = (group.config.options ?? []).map((o) => o.id);
    return out.sort((a, b) => (order.indexOf(a.key) + 1 || 999) - (order.indexOf(b.key) + 1 || 999));
  }
  if (group && ["date", "created_time", "last_edited_time"].includes(group.type)) return out.sort((a, b) => a.key.localeCompare(b.key));
  return out.sort((a, b) => b.value - a.value);
}

const fmt = (n: number, locale: string) => n.toLocaleString(locale, { maximumFractionDigits: 2 });

export function ChartView({ db, view, records }: ViewProps) {
  const { t, locale } = useI18n();
  const [asTable, setAsTable] = useState(false);
  const [tip, setTip] = useState<{ x: number; y: number; text: string } | null>(null);
  const wrap = useRef<HTMLDivElement>(null);
  const group = db.properties.find((p) => p.id === view.config.groupBy);
  const agg = view.config.aggregate ?? "count";
  const valueProp = db.properties.find((p) => p.id === view.config.valueProperty);
  const data = useMemo(() => buckets(records, group, agg, valueProp, t), [records, group, agg, valueProp, t]);
  const overTime = !!group && ["date", "created_time", "last_edited_time"].includes(group.type) && data.filter((d) => d.key).length > 1;
  const chartType = overTime && view.config.chartType !== "bar" ? "line" : "bar";
  const measure = agg === "count" ? t("Records") : `${t(agg === "sum" ? "Sum of" : "Average of")} ${valueProp?.name ?? "?"}`;
  const title = `${measure}${group ? ` ${t("by")} ${group.name}` : ""}`;
  const max = Math.max(1, ...data.map((d) => d.value));
  const show = (e: React.MouseEvent, text: string) => {
    const r = wrap.current?.getBoundingClientRect();
    if (r) setTip({ x: e.clientX - r.left + 12, y: e.clientY - r.top + 12, text });
  };

  if (agg !== "count" && !valueProp) return <div className="empty">{t("Choose a number property to sum or average in the chart settings above.")}</div>;
  if (!records.length) return <div className="empty">{t("No records to chart.")}</div>;

  return (
    <div className="chart-wrap" ref={wrap} onMouseLeave={() => setTip(null)}>
      <div className="row" style={{ marginBottom: 8 }}>
        <strong>{title}</strong>
        <span className="small-text faint">· {t("{n} records", { n: records.length })}</span>
        <span className="spacer" />
        <button className="btn small ghost" onClick={() => setAsTable(!asTable)} aria-pressed={asTable}>{asTable ? t("Show chart") : t("Show as table")}</button>
      </div>
      {asTable ? (
        <table className="plain">
          <thead><tr><th>{group?.name ?? ""}</th><th style={{ textAlign: "right" }}>{measure}</th><th style={{ textAlign: "right" }}>{t("Records")}</th></tr></thead>
          <tbody>{data.map((d) => <tr key={d.key}><td>{d.label}</td><td style={{ textAlign: "right" }}>{fmt(d.value, locale)}</td><td style={{ textAlign: "right" }}>{d.count}</td></tr>)}</tbody>
        </table>
      ) : chartType === "bar" ? (
        <div role="img" aria-label={title}>
          {data.map((d) => (
            <div key={d.key} className="row" style={{ height: 30, gap: 10 }} onMouseMove={(e) => show(e, `${d.label}: ${fmt(d.value, locale)}${agg !== "count" ? ` (${d.count} ${t("records")})` : ""}`)}>
              <span className="small-text" style={{ width: 160, textAlign: "right", overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap", color: "var(--text-muted)" }}>{d.label}</span>
              <div style={{ flex: 1, height: 18, position: "relative" }}>
                {/* Thin bar anchored at the baseline; rounded only at the data end. */}
                <div style={{ position: "absolute", left: 0, top: 3, height: 12, width: `${(d.value / max) * 100}%`, minWidth: d.value > 0 ? 2 : 0, background: "var(--series-1)", borderRadius: "0 4px 4px 0" }} />
              </div>
              <span className="small-text" style={{ width: 70, color: "var(--text)" }}>{fmt(d.value, locale)}</span>
            </div>
          ))}
        </div>
      ) : (
        <LineChart data={data.filter((d) => d.key)} max={max} title={title} onHover={(e, d) => show(e, `${d.label}: ${fmt(d.value, locale)}`)} />
      )}
      {tip && <div className="chart-tooltip" style={{ left: tip.x, top: tip.y }}>{tip.text}</div>}
    </div>
  );
}

function LineChart({ data, max, title, onHover }: { data: Bucket[]; max: number; title: string; onHover: (e: React.MouseEvent, d: Bucket) => void }) {
  const [hover, setHover] = useState<number | null>(null);
  const W = 720, H = 240, L = 44, B = 26, T = 10, R = 12;
  const x = (i: number) => L + (data.length === 1 ? (W - L - R) / 2 : (i * (W - L - R)) / (data.length - 1));
  const y = (v: number) => T + (H - T - B) * (1 - v / max);
  const ticks = [0, 0.5, 1].map((f) => Math.round(max * f * 100) / 100);
  const path = data.map((d, i) => `${i ? "L" : "M"} ${x(i)} ${y(d.value)}`).join(" ");
  return (
    <svg viewBox={`0 0 ${W} ${H}`} width="100%" role="img" aria-label={title}
      onMouseMove={(e) => {
        const r = (e.currentTarget as SVGSVGElement).getBoundingClientRect();
        const px = ((e.clientX - r.left) / r.width) * W;
        let best = 0;
        data.forEach((_, i) => { if (Math.abs(x(i) - px) < Math.abs(x(best) - px)) best = i; });
        setHover(best);
        onHover(e, data[best]);
      }}
      onMouseLeave={() => setHover(null)}>
      {ticks.map((tk) => (
        <g key={tk}>
          <line className="chart-grid" x1={L} x2={W - R} y1={y(tk)} y2={y(tk)} />
          <text className="chart-label" x={L - 6} y={y(tk) + 4} textAnchor="end">{tk}</text>
        </g>
      ))}
      <line className="chart-axis" x1={L} x2={W - R} y1={H - B} y2={H - B} />
      {data.map((d, i) => (i % Math.ceil(data.length / 8) === 0 || i === data.length - 1) && <text key={d.key} className="chart-label" x={x(i)} y={H - 8} textAnchor="middle">{d.label}</text>)}
      {hover !== null && <line x1={x(hover)} x2={x(hover)} y1={T} y2={H - B} stroke="var(--text-faint)" strokeWidth={1} />}
      <path d={path} fill="none" stroke="var(--series-1)" strokeWidth={2} strokeLinejoin="round" strokeLinecap="round" />
      {data.map((d, i) => <circle key={d.key} cx={x(i)} cy={y(d.value)} r={hover === i ? 5 : 4} fill="var(--series-1)" stroke="var(--bg)" strokeWidth={2} />)}
    </svg>
  );
}
