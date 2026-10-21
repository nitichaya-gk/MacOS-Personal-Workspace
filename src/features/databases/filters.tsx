import { useState } from "react";
import { useI18n } from "../../app/providers/i18n";
import type { DatabaseInfo, FilterCondition, FilterGroup, Property, SortSpec, ViewConfig } from "../../shared/types";
import { Icon } from "../../components/ui/Icon";
import { typeIcon } from "./cells";

type Op = { id: string; label: string; noValue?: boolean };

export function operatorsFor(p: Property | undefined): Op[] {
  const empty: Op[] = [{ id: "is_empty", label: "is empty", noValue: true }, { id: "is_not_empty", label: "is not empty", noValue: true }];
  switch (p?.type) {
    case "number":
      return [{ id: "eq", label: "=" }, { id: "neq", label: "≠" }, { id: "gt", label: ">" }, { id: "gte", label: "≥" }, { id: "lt", label: "<" }, { id: "lte", label: "≤" }, ...empty];
    case "select":
    case "status":
      return [{ id: "is", label: "is" }, { id: "is_not", label: "is not" }, ...empty];
    case "multi_select":
      return [{ id: "contains", label: "contains" }, { id: "not_contains", label: "does not contain" }, ...empty];
    case "checkbox":
      return [{ id: "is", label: "is" }];
    case "date":
    case "created_time":
    case "last_edited_time":
      return [{ id: "is", label: "is" }, { id: "before", label: "is before" }, { id: "after", label: "is after" }, { id: "on_or_before", label: "is on or before" }, { id: "on_or_after", label: "is on or after" }, ...empty];
    case "files":
      return empty;
    case "relation":
      return [{ id: "contains", label: "contains record" }, { id: "not_contains", label: "does not contain record" }, ...empty];
    case "formula":
    case "rollup":
      return [{ id: "eq", label: "=" }, { id: "gt", label: ">" }, { id: "lt", label: "<" }, { id: "contains", label: "contains" }, { id: "equals", label: "is" }, { id: "is", label: "is (true/false, date)" }, { id: "before", label: "is before" }, { id: "after", label: "is after" }, ...empty];
    default:
      return [{ id: "contains", label: "contains" }, { id: "not_contains", label: "does not contain" }, { id: "equals", label: "is" }, { id: "not_equals", label: "is not" }, { id: "starts_with", label: "starts with" }, { id: "ends_with", label: "ends with" }, ...empty];
  }
}

const isGroup = (c: FilterCondition | FilterGroup): c is FilterGroup => "conditions" in c;

export function countConditions(g?: FilterGroup): number {
  if (!g) return 0;
  return g.conditions.reduce((n, c) => n + (isGroup(c) ? countConditions(c) : 1), 0);
}

function ValueInput({ p, cond, onChange }: { p: Property | undefined; cond: FilterCondition; onChange: (v: unknown) => void }) {
  const { t } = useI18n();
  const op = operatorsFor(p).find((o) => o.id === cond.operator);
  if (!p || op?.noValue) return <span />;
  if (p.type === "select" || p.type === "status" || p.type === "multi_select")
    return (
      <select className="select" value={String(cond.value ?? "")} onChange={(e) => onChange(e.target.value)}>
        <option value="">{t("Choose…")}</option>
        {(p.config.options ?? []).map((o) => <option key={o.id} value={o.id}>{o.name}</option>)}
      </select>
    );
  if (p.type === "checkbox")
    return (
      <select className="select" value={String(cond.value ?? true)} onChange={(e) => onChange(e.target.value === "true")}>
        <option value="true">{t("Checked")}</option>
        <option value="false">{t("Unchecked")}</option>
      </select>
    );
  if (p.type === "date" || p.type === "created_time" || p.type === "last_edited_time")
    return <input className="input" type="date" value={String(cond.value ?? "")} onChange={(e) => onChange(e.target.value)} />;
  return (
    <input className="input" value={String(cond.value ?? "")} inputMode={p.type === "number" ? "decimal" : undefined} placeholder={t("Value")}
      onChange={(e) => onChange(p.type === "number" ? (e.target.value === "" ? "" : Number(e.target.value)) : e.target.value)} />
  );
}

function GroupEditor({ db, group, onChange, depth }: { db: DatabaseInfo; group: FilterGroup; onChange: (g: FilterGroup) => void; depth: number }) {
  const { t } = useI18n();
  const props = db.properties;
  const update = (i: number, c: FilterCondition | FilterGroup | null) => {
    const next = [...group.conditions];
    if (c === null) next.splice(i, 1);
    else next[i] = c;
    onChange({ ...group, conditions: next });
  };
  const addCondition = () => {
    const p = props[0];
    onChange({ ...group, conditions: [...group.conditions, { propertyId: p.id, operator: operatorsFor(p)[0].id, value: "" }] });
  };
  return (
    <div style={depth ? { border: "1px solid var(--border)", borderRadius: 6, padding: 8, background: "var(--bg-subtle)" } : undefined}>
      {group.conditions.map((c, i) => {
        const lead =
          i === 0 ? (
            <span className="small-text muted">{t("Where")}</span>
          ) : i === 1 ? (
            <select className="select" value={group.op} onChange={(e) => onChange({ ...group, op: e.target.value as "and" | "or" })} aria-label={t("Combine with")}>
              <option value="and">{t("And")}</option>
              <option value="or">{t("Or")}</option>
            </select>
          ) : (
            <span className="small-text muted">{group.op === "and" ? t("And") : t("Or")}</span>
          );
        if (isGroup(c))
          return (
            <div key={i} style={{ display: "grid", gridTemplateColumns: "56px 1fr 28px", gap: 6, marginBottom: 6, alignItems: "start" }}>
              {lead}
              <GroupEditor db={db} group={c} onChange={(g) => update(i, g)} depth={depth + 1} />
              <button className="icon-btn small" aria-label={t("Remove group")} onClick={() => update(i, null)}><Icon name="close" size={12} /></button>
            </div>
          );
        const p = props.find((x) => x.id === c.propertyId);
        return (
          <div key={i} className="filter-row">
            {lead}
            <select className="select" value={c.propertyId} aria-label={t("Property")}
              onChange={(e) => { const np = props.find((x) => x.id === e.target.value); update(i, { propertyId: e.target.value, operator: operatorsFor(np)[0].id, value: "" }); }}>
              {props.map((x) => <option key={x.id} value={x.id}>{typeIcon(x.type)} {x.name}</option>)}
            </select>
            <select className="select" value={c.operator} onChange={(e) => update(i, { ...c, operator: e.target.value })} aria-label={t("Condition")}>
              {operatorsFor(p).map((o) => <option key={o.id} value={o.id}>{t(o.label)}</option>)}
            </select>
            <ValueInput p={p} cond={c} onChange={(v) => update(i, { ...c, value: v })} />
            <button className="icon-btn small" aria-label={t("Remove filter")} onClick={() => update(i, null)}><Icon name="close" size={12} /></button>
          </div>
        );
      })}
      <div className="row" style={{ marginTop: 4 }}>
        <button className="btn small ghost" onClick={addCondition}>＋ {t("Add filter")}</button>
        {depth === 0 && (
          <button className="btn small ghost" onClick={() => onChange({ ...group, conditions: [...group.conditions, { op: "and", conditions: [{ propertyId: props[0].id, operator: operatorsFor(props[0])[0].id, value: "" }] }] })}>
            ＋ {t("Add filter group")}
          </button>
        )}
      </div>
    </div>
  );
}

export function FilterEditor({ db, config, onChange, initialProperty }: { db: DatabaseInfo; config: ViewConfig; onChange: (f: FilterGroup) => void; initialProperty?: string }) {
  const { t } = useI18n();
  const [group, setGroup] = useState<FilterGroup>(() => {
    const g = config.filter ?? { op: "and", conditions: [] };
    if (initialProperty) {
      const p = db.properties.find((x) => x.id === initialProperty);
      return { ...g, conditions: [...g.conditions, { propertyId: initialProperty, operator: operatorsFor(p)[0].id, value: "" }] };
    }
    return g;
  });
  const change = (g: FilterGroup) => {
    setGroup(g);
    onChange(g);
  };
  return (
    <div style={{ padding: 8, width: 600, maxWidth: "calc(100vw - 40px)" }}>
      {group.conditions.length === 0 && <div className="muted small-text" style={{ marginBottom: 8 }}>{t("No filters. Records matching all filters are shown.")}</div>}
      <GroupEditor db={db} group={group} onChange={change} depth={0} />
    </div>
  );
}

export function SortEditor({ db, config, onChange }: { db: DatabaseInfo; config: ViewConfig; onChange: (s: SortSpec[]) => void }) {
  const { t } = useI18n();
  const [sorts, setSorts] = useState<SortSpec[]>(config.sorts ?? []);
  const change = (s: SortSpec[]) => (setSorts(s), onChange(s));
  return (
    <div style={{ padding: 8, width: 420 }}>
      {sorts.length === 0 && <div className="muted small-text" style={{ marginBottom: 8 }}>{t("No sorting. Records keep their manual order.")}</div>}
      {sorts.map((s, i) => (
        <div key={i} className="row" style={{ marginBottom: 6 }}>
          <span className="small-text muted" style={{ width: 40 }}>{i === 0 ? t("Sort") : t("then")}</span>
          <select className="select" value={s.propertyId} onChange={(e) => change(sorts.map((x, j) => (j === i ? { ...x, propertyId: e.target.value } : x)))}>
            {db.properties.map((p) => <option key={p.id} value={p.id}>{p.name}</option>)}
          </select>
          <select className="select" style={{ width: 130 }} value={s.direction} onChange={(e) => change(sorts.map((x, j) => (j === i ? { ...x, direction: e.target.value as "asc" | "desc" } : x)))}>
            <option value="asc">{t("Ascending")}</option>
            <option value="desc">{t("Descending")}</option>
          </select>
          <button className="icon-btn small" aria-label={t("Move up")} disabled={i === 0} onClick={() => { const n = [...sorts]; [n[i - 1], n[i]] = [n[i], n[i - 1]]; change(n); }}>↑</button>
          <button className="icon-btn small" aria-label={t("Remove sort")} onClick={() => change(sorts.filter((_, j) => j !== i))}><Icon name="close" size={12} /></button>
        </div>
      ))}
      <button className="btn small ghost" onClick={() => change([...sorts, { propertyId: db.properties.find((p) => !sorts.some((x) => x.propertyId === p.id))?.id ?? db.properties[0].id, direction: "asc" }])}>
        ＋ {t("Add sort")}
      </button>
    </div>
  );
}

export function PropertiesEditor({ db, config, onChange }: { db: DatabaseInfo; config: ViewConfig; onChange: (hidden: string[]) => void }) {
  const { t } = useI18n();
  const [hidden, setHidden] = useState<string[]>(config.hidden ?? []);
  return (
    <div style={{ padding: 4, width: 260 }}>
      <div className="menu-label">{t("Shown in this view")}</div>
      {db.properties.map((p) => (
        <label key={p.id} className="menu-item">
          <input
            type="checkbox"
            disabled={p.type === "title"}
            checked={!hidden.includes(p.id)}
            onChange={(e) => {
              const next = e.target.checked ? hidden.filter((h) => h !== p.id) : [...hidden, p.id];
              setHidden(next);
              onChange(next);
            }}
          />
          <span style={{ width: 18, textAlign: "center" }} className="muted">{typeIcon(p.type)}</span>
          {p.name}
        </label>
      ))}
    </div>
  );
}
