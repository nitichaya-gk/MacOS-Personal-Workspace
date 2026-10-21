import { useMemo, useState } from "react";
import { useStore } from "../../app/providers/store";
import { useI18n } from "../../app/providers/i18n";
import { api } from "../../services/desktop-bridge/api";
import type { DatabaseInfo, DbRecord, Property, View } from "../../shared/types";
import { addDays, clsx, dateInZone } from "../../shared/utils";
import { Icon } from "../../components/ui/Icon";
import { AddPropertyButton, OptionChip, PropertyCell, PropertyValue, typeIcon, valueOf } from "./cells";
import type { Anchor } from "../../components/ui/Popover";

export interface ViewProps {
  db: DatabaseInfo;
  view: View;
  records: DbRecord[];
  total: number;
  reload: () => void;
  replace: (r: DbRecord) => void;
  create: (values?: Record<string, unknown>, title?: string) => Promise<void>;
  open: (r: DbRecord) => void;
  loadMore: () => void;
  openPropMenu: (anchor: Anchor, p: Property) => void;
}

export const visibleProps = (db: DatabaseInfo, view: View) => db.properties.filter((p) => p.type === "title" || !(view.config.hidden ?? []).includes(p.id));

export function TableView({ db, view, records, total, reload, replace, create, open, loadMore, openPropMenu }: ViewProps) {
  const s = useStore();
  const { t } = useI18n();
  const props = visibleProps(db, view);
  const [dragId, setDragId] = useState<string | null>(null);
  const [overId, setOverId] = useState<string | null>(null);
  const manual = !(view.config.sorts?.length);

  const drop = async (beforeId: string | null) => {
    const id = dragId;
    setDragId(null);
    setOverId(null);
    if (!id || id === beforeId) return;
    await api.databases.moveRecord(id, beforeId).then(reload).catch(s.fail);
  };

  return (
    <div className="table-wrap">
      <table className="db" style={{ width: props.reduce((w, p) => w + (p.type === "title" ? 280 : 190), 0) + 48 }}>
        <colgroup>
          {props.map((p) => <col key={p.id} style={{ width: p.type === "title" ? 280 : 190 }} />)}
          <col style={{ width: 48 }} />
        </colgroup>
        <thead>
          <tr>
            {props.map((p) => (
              <th key={p.id} scope="col">
                <button className="th" onClick={(e) => openPropMenu(e.currentTarget, p)} aria-label={t("{name} column menu", { name: p.name })}>
                  <span className="muted" style={{ fontSize: 12 }}>{typeIcon(p.type)}</span>
                  <span>{p.name}</span>
                </button>
              </th>
            ))}
            <th><AddPropertyButton db={db} onAdded={reload} /></th>
          </tr>
        </thead>
        <tbody>
          {records.map((r) => (
            <tr
              key={r.id}
              className={clsx(dragId === r.id && "dragging", overId === r.id && "drop-target")}
              draggable={manual}
              onDragStart={(e) => (setDragId(r.id), (e.dataTransfer.effectAllowed = "move"))}
              onDragOver={(e) => dragId && (e.preventDefault(), setOverId(r.id))}
              onDrop={(e) => (e.preventDefault(), drop(r.id))}
              onDragEnd={() => (setDragId(null), setOverId(null))}
            >
              {props.map((p) => (
                <td key={p.id}>
                  <PropertyCell db={db} p={p} rec={r} onSaved={replace} className={p.type === "title" ? "title-cell" : undefined}>
                    {p.type === "title" && (
                      <>
                        {!r.title && <span className="faint">{t("Untitled")}</span>}
                        <button className="btn small open-btn" onClick={(e) => (e.stopPropagation(), open(r))}>{t("Open")}</button>
                      </>
                    )}
                  </PropertyCell>
                </td>
              ))}
              <td />
            </tr>
          ))}
        </tbody>
      </table>
      {records.length < total && <button className="add-row" onClick={loadMore}>{t("Load more ({n} remaining)", { n: total - records.length })}</button>}
      <button className="add-row" onClick={() => create()} onDragOver={(e) => dragId && e.preventDefault()} onDrop={() => drop(null)}>＋ {t("New")}</button>
    </div>
  );
}

export function BoardView({ db, view, records, replace, create, open, reload }: ViewProps) {
  const s = useStore();
  const { t } = useI18n();
  const group = db.properties.find((p) => p.id === view.config.groupBy && ["select", "status"].includes(p.type));
  const [drag, setDrag] = useState<string | null>(null);
  const [over, setOver] = useState<string | null>(null);
  const shown = visibleProps(db, view).filter((p) => p.type !== "title").slice(0, 4);

  if (!group) {
    const candidates = db.properties.filter((p) => p.type === "select" || p.type === "status");
    return (
      <div className="empty">
        <div>{t("Board views group records by a Select or Status property.")}</div>
        <div className="actions">
          {candidates.map((p) => (
            <button key={p.id} className="btn" onClick={() => api.databases.updateView(view.id, { config: { ...view.config, groupBy: p.id } }).then(reload).catch(s.fail)}>
              {t("Group by {name}", { name: p.name })}
            </button>
          ))}
          {!candidates.length && <button className="btn" onClick={() => api.databases.addProperty(db.id, "Status", "status").then(reload).catch(s.fail)}>{t("Add a Status property")}</button>}
        </div>
      </div>
    );
  }
  const columns = [...(group.config.options ?? []).map((o) => ({ id: o.id, option: o })), { id: "", option: undefined }];
  const move = async (colId: string) => {
    const id = drag;
    setDrag(null);
    setOver(null);
    if (!id) return;
    const r = records.find((x) => x.id === id);
    if (!r || (valueOf(r, group) ?? "") === colId) return;
    api.databases.setValue(id, group.id, colId || null).then(replace).catch(s.fail);
  };
  return (
    <div className="board">
      {columns.map((c) => {
        const items = records.filter((r) => (valueOf(r, group) ?? "") === c.id);
        return (
          <div
            key={c.id || "none"}
            className={clsx("board-col", over === c.id && "drop")}
            onDragOver={(e) => drag && (e.preventDefault(), setOver(c.id))}
            onDragLeave={() => setOver(null)}
            onDrop={(e) => (e.preventDefault(), move(c.id))}
            aria-label={c.option?.name ?? t("No {name}", { name: group.name })}
          >
            <div className="board-col-head">
              {c.option ? <OptionChip option={c.option} /> : <span className="muted">{t("No {name}", { name: group.name })}</span>}
              <span className="faint">{items.length}</span>
            </div>
            {items.map((r) => (
              <div key={r.id} className="card" draggable onDragStart={() => setDrag(r.id)} onDragEnd={() => (setDrag(null), setOver(null))} onClick={() => open(r)} role="button" tabIndex={0} onKeyDown={(e) => e.key === "Enter" && open(r)}>
                <div className="card-title">{r.icon ? `${r.icon} ` : ""}{r.title || <span className="faint">{t("Untitled")}</span>}</div>
                <div className="card-props">
                  {shown.filter((p) => p.id !== group.id).map((p) => <PropertyValue key={p.id} p={p} rec={r} />)}
                </div>
              </div>
            ))}
            <button className="add-row" onClick={() => create(c.id ? { [group.id]: c.id } : {})}>＋ {t("New")}</button>
          </div>
        );
      })}
    </div>
  );
}

export function CalendarView({ db, view, records, replace, create, open, reload }: ViewProps) {
  const s = useStore();
  const { t, locale } = useI18n();
  const tz = s.settings?.effectiveTimezone ?? Intl.DateTimeFormat().resolvedOptions().timeZone;
  const today = dateInZone(new Date(), tz);
  const [month, setMonth] = useState(today.slice(0, 7));
  const [drag, setDrag] = useState<string | null>(null);
  const [over, setOver] = useState<string | null>(null);
  const dateProp = db.properties.find((p) => p.id === view.config.dateProperty && p.type === "date") ?? db.properties.find((p) => p.type === "date");

  const days = useMemo(() => {
    const first = `${month}-01`;
    const dow = (new Date(`${first}T00:00:00Z`).getUTCDay() + 6) % 7; // Monday first
    const start = addDays(first, -dow);
    return Array.from({ length: 42 }, (_, i) => addDays(start, i));
  }, [month]);

  if (!dateProp)
    return (
      <div className="empty">
        <div>{t("Calendar views need a Date property.")}</div>
        <div className="actions"><button className="btn" onClick={() => api.databases.addProperty(db.id, "Date", "date").then(reload).catch(s.fail)}>{t("Add a Date property")}</button></div>
      </div>
    );

  const byDay = new Map<string, DbRecord[]>();
  for (const r of records) {
    const d = valueOf(r, dateProp) as { start: string; end?: string } | null;
    if (!d?.start) continue;
    const start = d.start.slice(0, 10);
    const end = d.end?.slice(0, 10) ?? start;
    for (let day = start, n = 0; day <= end && n < 62; day = addDays(day, 1), n++) {
      if (!byDay.has(day)) byDay.set(day, []);
      byDay.get(day)!.push(r);
    }
  }
  const shift = (n: number) => {
    const [y, m] = month.split("-").map(Number);
    const d = new Date(Date.UTC(y, m - 1 + n, 1));
    setMonth(d.toISOString().slice(0, 7));
  };
  const moveTo = (day: string) => {
    const id = drag;
    setDrag(null);
    setOver(null);
    const r = records.find((x) => x.id === id);
    if (!r) return;
    const d = valueOf(r, dateProp) as { start: string; end?: string } | null;
    const len = d?.end ? Math.round((Date.parse(d.end) - Date.parse(d.start)) / 86400000) : 0;
    api.databases.setValue(r.id, dateProp.id, len ? { start: day, end: addDays(day, len) } : { start: day }).then(replace).catch(s.fail);
  };
  const weekdays = Array.from({ length: 7 }, (_, i) => new Date(Date.UTC(2024, 0, 1 + i)).toLocaleDateString(locale, { weekday: "short", timeZone: "UTC" }));
  const label = new Date(`${month}-01T00:00:00Z`).toLocaleDateString(locale, { month: "long", year: "numeric", timeZone: "UTC" });

  return (
    <div>
      <div className="row" style={{ margin: "6px 0 10px" }}>
        <strong style={{ fontSize: 16 }}>{label}</strong>
        <span className="spacer" />
        <span className="small-text muted">{t("By {name}", { name: dateProp.name })}</span>
        <button className="icon-btn" aria-label={t("Previous month")} onClick={() => shift(-1)}><Icon name="chevronLeft" /></button>
        <button className="btn small" onClick={() => setMonth(today.slice(0, 7))}>{t("Today")}</button>
        <button className="icon-btn" aria-label={t("Next month")} onClick={() => shift(1)}><Icon name="chevronRight" /></button>
      </div>
      <div className="calendar">
        <div className="cal-head">{weekdays.map((w) => <div key={w}>{w}</div>)}</div>
        <div className="cal-grid">
          {days.map((day) => (
            <div
              key={day}
              className={clsx("cal-day", !day.startsWith(month) && "other", day === today && "today", over === day && "drop")}
              onDragOver={(e) => drag && (e.preventDefault(), setOver(day))}
              onDrop={(e) => (e.preventDefault(), moveTo(day))}
            >
              <span className="num">{Number(day.slice(8))}</span>
              <button className="icon-btn small add" aria-label={t("New on {date}", { date: day })} onClick={() => create({ [dateProp.id]: { start: day } })}><Icon name="plus" size={12} /></button>
              {(byDay.get(day) ?? []).map((r) => (
                <div key={r.id} className="cal-item" draggable onDragStart={() => setDrag(r.id)} onClick={() => open(r)} title={r.title}>
                  {r.icon ? `${r.icon} ` : ""}{r.title || t("Untitled")}
                </div>
              ))}
            </div>
          ))}
        </div>
      </div>
      {records.filter((r) => !valueOf(r, dateProp)).length > 0 && (
        <div className="small-text muted" style={{ marginTop: 8 }}>{t("{n} records have no date and are not shown.", { n: records.filter((r) => !valueOf(r, dateProp)).length })}</div>
      )}
    </div>
  );
}

export function ListView({ db, view, records, total, create, open, loadMore }: ViewProps) {
  const { t } = useI18n();
  const shown = visibleProps(db, view).filter((p) => p.type !== "title").slice(0, 4);
  return (
    <div>
      {records.map((r) => (
        <div key={r.id} className="list-row" onClick={() => open(r)} role="button" tabIndex={0} onKeyDown={(e) => e.key === "Enter" && open(r)}>
          <span>{r.icon || "📄"}</span>
          <span className="t">{r.title || <span className="faint">{t("Untitled")}</span>}</span>
          <span className="row" style={{ gap: 6 }}>{shown.map((p) => <PropertyValue key={p.id} p={p} rec={r} />)}</span>
        </div>
      ))}
      {records.length < total && <button className="add-row" onClick={loadMore}>{t("Load more ({n} remaining)", { n: total - records.length })}</button>}
      <button className="add-row" onClick={() => create()}>＋ {t("New")}</button>
    </div>
  );
}
