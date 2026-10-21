import { useEffect, useRef, useState, type ReactNode } from "react";
import { useStore } from "../../app/providers/store";
import { useI18n } from "../../app/providers/i18n";
import { api } from "../../services/desktop-bridge/api";
import type { DatabaseInfo, DbRecord, Property, PropertyType, SelectOption } from "../../shared/types";
import { formatDate, formatDateTime } from "../../shared/utils";
import { Popover, Menu, useAnchor, type Anchor, type MenuEntry } from "../../components/ui/Popover";
import { Icon } from "../../components/ui/Icon";
import { Modal } from "../../components/ui/Modal";

export const PROPERTY_TYPES: { type: PropertyType; label: string; icon: string }[] = [
  { type: "text", label: "Text", icon: "≡" },
  { type: "number", label: "Number", icon: "#" },
  { type: "select", label: "Select", icon: "◉" },
  { type: "multi_select", label: "Multi-select", icon: "☰" },
  { type: "status", label: "Status", icon: "◐" },
  { type: "date", label: "Date", icon: "📅" },
  { type: "checkbox", label: "Checkbox", icon: "☑" },
  { type: "url", label: "URL", icon: "🔗" },
  { type: "email", label: "Email", icon: "@" },
  { type: "files", label: "Files & media", icon: "📎" },
  { type: "created_time", label: "Created time", icon: "🕒" },
  { type: "last_edited_time", label: "Last edited time", icon: "🕓" },
  { type: "relation", label: "Relation", icon: "↗" },
  { type: "formula", label: "Formula", icon: "ƒ" },
  { type: "rollup", label: "Rollup", icon: "Σ" },
];

export const typeIcon = (t: PropertyType) => (t === "title" ? "Aa" : PROPERTY_TYPES.find((p) => p.type === t)?.icon ?? "≡");
const COLORS = ["gray", "brown", "orange", "yellow", "green", "blue", "purple", "pink", "red"];

export function valueOf(rec: DbRecord, p: Property): unknown {
  if (p.type === "title") return rec.title;
  if (p.type === "created_time") return rec.createdAt;
  if (p.type === "last_edited_time") return rec.updatedAt;
  return rec.values[p.id] ?? null;
}

export function optionOf(p: Property, id: unknown): SelectOption | undefined {
  return p.config.options?.find((o) => o.id === id);
}

export function OptionChip({ option }: { option: SelectOption | undefined }) {
  if (!option) return null;
  const marker = option.group === "done" ? "● " : option.group === "in_progress" ? "◐ " : option.group === "todo" ? "○ " : "";
  // Status also shows a shape, so meaning never depends on colour alone.
  return <span className={`chip c-${option.color}`}>{marker}{option.name}</span>;
}

/** Read-only rendering of a value. */
export function PropertyValue({ p, rec }: { p: Property; rec: DbRecord }) {
  const { locale } = useI18n();
  const v = valueOf(rec, p);
  if (v === null || v === undefined || v === "") return null;
  if (typeof v === "object" && v && "error" in (v as object)) {
    return <span className="chip c-red" title={String((v as { error: string }).error)}>⚠ {String((v as { error: string }).error).slice(0, 40)}</span>;
  }
  switch (p.type) {
    case "relation":
      return <>{(v as string[]).map((id) => rec.relationTitles?.[id] !== undefined && <RelationChip key={id} id={id} title={rec.relationTitles[id]} />)}</>;
    case "formula":
    case "rollup": {
      if (typeof v === "number") return <span style={{ marginLeft: "auto" }}>{Number.isInteger(v) ? v.toLocaleString(locale) : v.toLocaleString(locale, { maximumFractionDigits: 4 })}</span>;
      if (typeof v === "boolean") return <span>{v ? "☑" : "☐"}</span>;
      if (Array.isArray(v)) return <span>{v.map((x) => (typeof x === "object" && x && "start" in x ? (x as { start: string }).start : String(x))).join(", ")}</span>;
      if (typeof v === "object" && v && "start" in v) return <span>{formatDate((v as { start: string }).start, locale)}</span>;
      return <span>{String(v)}</span>;
    }
    case "select":
    case "status":
      return <OptionChip option={optionOf(p, v)} />;
    case "multi_select":
      return <>{(v as string[]).map((id) => <OptionChip key={id} option={optionOf(p, id)} />)}</>;
    case "date": {
      const d = v as { start: string; end?: string };
      return <span>{formatDate(d.start, locale)}{d.end ? ` → ${formatDate(d.end, locale)}` : ""}</span>;
    }
    case "checkbox":
      return <span aria-label={v ? "checked" : "unchecked"}>{v ? "☑" : "☐"}</span>;
    case "created_time":
    case "last_edited_time":
      return <span className="muted">{formatDateTime(v as string, locale)}</span>;
    case "files":
      return <>{(v as { assetId: string; name: string }[]).map((f) => <span key={f.assetId} className="chip">📎 {f.name}</span>)}</>;
    case "url":
      return (
        <a href="#" onClick={(e) => (e.preventDefault(), e.stopPropagation(), api.system.openExternal(String(v)))} style={{ overflow: "hidden", textOverflow: "ellipsis" }}>
          {String(v)}
        </a>
      );
    case "number":
      return <span style={{ marginLeft: "auto" }}>{Number(v).toLocaleString(locale)}</span>;
    default:
      return <span style={{ whiteSpace: "pre-wrap", wordBreak: "break-word" }}>{String(v)}</span>;
  }
}

function RelationChip({ id, title }: { id: string; title: string }) {
  const s = useStore();
  return (
    <span className="chip" role="link" style={{ cursor: "default", textDecoration: "underline", textDecorationColor: "var(--border-strong)" }}
      onClick={(e) => (e.stopPropagation(), s.navigate({ name: "page", id }))}>↗ {title || "Untitled"}</span>
  );
}

/** Editable cell: click opens the right editor for the property type. */
export function PropertyCell({ db, p, rec, onSaved, children, className }: { db: DatabaseInfo; p: Property; rec: DbRecord; onSaved: (r: DbRecord) => void; children?: ReactNode; className?: string }) {
  const s = useStore();
  const { t } = useI18n();
  const pop = useAnchor();
  const ref = useRef<HTMLDivElement>(null);
  const computed = ["created_time", "last_edited_time", "formula", "rollup"].includes(p.type);
  const set = (value: unknown) => api.databases.setValue(rec.id, p.id, value).then(onSaved).catch(s.fail);

  if (p.type === "checkbox")
    return (
      <div className={`cell ${className ?? ""}`}>
        <input type="checkbox" checked={!!valueOf(rec, p)} onChange={(e) => set(e.target.checked)} aria-label={p.name} />
      </div>
    );

  return (
    <>
      <div
        ref={ref}
        className={`cell ${className ?? ""}`}
        tabIndex={computed ? -1 : 0}
        onClick={() => !computed && pop.open(ref.current!)}
        onKeyDown={(e) => e.key === "Enter" && !computed && pop.open(ref.current!)}
        role={computed ? undefined : "button"}
        aria-label={`${p.name}`}
      >
        <PropertyValue p={p} rec={rec} />
        {children}
      </div>
      {pop.anchor && (
        <Popover anchor={pop.anchor} onClose={pop.close} width={Math.max(260, (ref.current?.offsetWidth ?? 0) + 2)}>
          <ValueEditor db={db} p={p} rec={rec} onCommit={(v) => (set(v), pop.close())} onLiveCommit={set} close={pop.close} t={t} />
        </Popover>
      )}
    </>
  );
}

function ValueEditor({ db, p, rec, onCommit, onLiveCommit, close, t }: {
  db: DatabaseInfo; p: Property; rec: DbRecord; onCommit: (v: unknown) => void; onLiveCommit: (v: unknown) => Promise<unknown>; close: () => void; t: (s: string, v?: Record<string, string | number>) => string;
}) {
  const s = useStore();
  const v = valueOf(rec, p);
  const [text, setText] = useState(v === null || v === undefined ? "" : typeof v === "object" ? "" : String(v));
  const [query, setQuery] = useState("");
  const [opts, setOpts] = useState<SelectOption[]>(p.config.options ?? []);

  if (["title", "text", "number", "url", "email"].includes(p.type)) {
    const multi = p.type === "text";
    const commit = () => onCommit(p.type === "number" ? (text.trim() === "" ? null : text) : text);
    return multi ? (
      <textarea className="textarea" autoFocus rows={3} value={text} onChange={(e) => setText(e.target.value)}
        onKeyDown={(e) => { if (e.key === "Enter" && !e.shiftKey && !e.nativeEvent.isComposing) (e.preventDefault(), commit()); }}
        onBlur={commit} />
    ) : (
      <input className="input" autoFocus value={text} inputMode={p.type === "number" ? "decimal" : undefined} type={p.type === "email" ? "email" : "text"}
        onChange={(e) => setText(e.target.value)}
        onKeyDown={(e) => { if (e.key === "Enter" && !e.nativeEvent.isComposing) commit(); }}
        onBlur={commit} />
    );
  }

  if (p.type === "select" || p.type === "status" || p.type === "multi_select") {
    const selected: string[] = p.type === "multi_select" ? ((v as string[]) ?? []) : v ? [v as string] : [];
    const q = query.trim().toLowerCase();
    const shown = opts.filter((o) => o.name.toLowerCase().includes(q));
    const toggle = (id: string) => {
      if (p.type === "multi_select") {
        const next = selected.includes(id) ? selected.filter((x) => x !== id) : [...selected, id];
        onLiveCommit(next.length ? next : null);
      } else onCommit(selected[0] === id ? null : id);
    };
    const create = async () => {
      if (!query.trim()) return;
      try {
        const r = await api.databases.addOption(p.id, query.trim());
        const fresh = await api.databases.get(db.id);
        setOpts(fresh.properties.find((x) => x.id === p.id)?.config.options ?? []);
        setQuery("");
        toggle(r.optionId);
      } catch (e) {
        s.fail(e);
      }
    };
    return (
      <div>
        <input className="input" autoFocus placeholder={t("Search or create an option")} value={query} onChange={(e) => setQuery(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Enter" && !e.nativeEvent.isComposing) {
              const exact = opts.find((o) => o.name.toLowerCase() === q);
              if (exact) toggle(exact.id);
              else if (shown.length === 1) toggle(shown[0].id);
              else create();
            }
          }} />
        <div style={{ marginTop: 4 }}>
          {shown.map((o) => (
            <button key={o.id} className="menu-item" onClick={() => toggle(o.id)} aria-pressed={selected.includes(o.id)}>
              <span style={{ width: 14 }}>{selected.includes(o.id) ? "✓" : ""}</span>
              <OptionChip option={o} />
            </button>
          ))}
          {query.trim() && !opts.some((o) => o.name.toLowerCase() === q) && (
            <button className="menu-item" onClick={create}>＋ {t("Create")} <span className="chip">{query.trim()}</span></button>
          )}
          {selected.length > 0 && (
            <>
              <div className="menu-sep" />
              <button className="menu-item" onClick={() => onCommit(null)}>{t("Clear")}</button>
            </>
          )}
        </div>
      </div>
    );
  }

  if (p.type === "relation") return <RelationEditor p={p} rec={rec} onCommit={onLiveCommit} t={t} />;

  if (p.type === "date") {
    const d = (v as { start: string; end?: string } | null) ?? null;
    return <DateEditor value={d} onCommit={onCommit} t={t} />;
  }

  if (p.type === "files") {
    const files = (v as { assetId: string; name: string }[] | null) ?? [];
    return (
      <div>
        {files.map((f) => (
          <div key={f.assetId} className="row" style={{ padding: "2px 4px" }}>
            <span className="spacer" style={{ overflow: "hidden", textOverflow: "ellipsis" }}>📎 {f.name}</span>
            <button className="icon-btn small" aria-label={t("Remove")} onClick={() => onLiveCommit(files.filter((x) => x.assetId !== f.assetId))}><Icon name="close" size={12} /></button>
          </div>
        ))}
        <label className="btn small" style={{ marginTop: 6 }}>
          <Icon name="upload" size={13} /> {t("Upload file")}
          <input type="file" hidden multiple onChange={async (e) => {
            const list = Array.from(e.target.files ?? []);
            try {
              const up = await Promise.all(list.map((f) => api.assets.upload(f, rec.id)));
              onCommit([...files, ...up.map((a) => ({ assetId: a.id, name: a.fileName }))]);
            } catch (err) {
              s.fail(err);
            }
          }} />
        </label>
      </div>
    );
  }
  return <div className="muted small-text" style={{ padding: 6 }} onClick={close}>{t("This value is set automatically.")}</div>;
}

function RelationEditor({ p, rec, onCommit, t }: { p: Property; rec: DbRecord; onCommit: (v: unknown) => Promise<unknown>; t: (s: string) => string }) {
  const s = useStore();
  const target = (p.config.databaseId as string) || "";
  const [q, setQ] = useState("");
  const [found, setFound] = useState<DbRecord[]>([]);
  const [selected, setSelected] = useState<string[]>((rec.values[p.id] as string[]) ?? []);
  const [titles, setTitles] = useState<Record<string, string>>(rec.relationTitles ?? {});
  useEffect(() => {
    if (!target) return;
    api.databases.query(target, { search: q || undefined, limit: 30 }).then((r) => setFound(r.records)).catch(s.fail);
  }, [q, target, s.fail]);
  if (!target) return <div className="small-text muted" style={{ padding: 6 }}>{t("Choose the related database in the column menu first.")}</div>;
  const toggle = (r: DbRecord) => {
    const next = selected.includes(r.id) ? selected.filter((x) => x !== r.id) : [...selected, r.id];
    setSelected(next);
    setTitles({ ...titles, [r.id]: r.title });
    onCommit(next.length ? next : null);
  };
  return (
    <div>
      <div className="row wrap" style={{ gap: 4, marginBottom: 6 }}>
        {selected.map((id) => <span key={id} className="chip">↗ {titles[id] ?? "…"}<button onClick={() => toggle({ id } as DbRecord)}>×</button></span>)}
      </div>
      <input className="input" autoFocus placeholder={t("Search records")} value={q} onChange={(e) => setQ(e.target.value)} />
      <div style={{ marginTop: 4, maxHeight: 240, overflowY: "auto" }}>
        {found.map((r) => (
          <button key={r.id} className="menu-item" onClick={() => toggle(r)} aria-pressed={selected.includes(r.id)}>
            <span style={{ width: 14 }}>{selected.includes(r.id) ? "✓" : ""}</span>{r.title || t("Untitled")}
          </button>
        ))}
      </div>
    </div>
  );
}

function DateEditor({ value, onCommit, t }: { value: { start: string; end?: string } | null; onCommit: (v: unknown) => void; t: (s: string) => string }) {
  const [start, setStart] = useState(value?.start?.slice(0, 10) ?? "");
  const [end, setEnd] = useState(value?.end?.slice(0, 10) ?? "");
  const [range, setRange] = useState(!!value?.end);
  const commit = () => onCommit(start ? (range && end ? { start, end } : { start }) : null);
  return (
    <div>
      <div className="field">
        <label>{range ? t("Start") : t("Date")}</label>
        <input className="input" type="date" autoFocus value={start} onChange={(e) => setStart(e.target.value)} onKeyDown={(e) => e.key === "Enter" && commit()} />
      </div>
      {range && (
        <div className="field">
          <label>{t("End")}</label>
          <input className="input" type="date" value={end} min={start} onChange={(e) => setEnd(e.target.value)} />
        </div>
      )}
      <label className="row small-text" style={{ marginBottom: 10 }}>
        <input type="checkbox" checked={range} onChange={(e) => setRange(e.target.checked)} /> {t("End date")}
      </label>
      <div className="row">
        <button className="btn small" onClick={() => onCommit(null)}>{t("Clear")}</button>
        <span className="spacer" />
        <button className="btn small" onClick={() => { const d = new Date(); setStart(`${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}-${String(d.getDate()).padStart(2, "0")}`); }}>{t("Today")}</button>
        <button className="btn small primary" onClick={commit}>{t("Done")}</button>
      </div>
    </div>
  );
}

/** Column header menu: rename, change type (with data-loss preview), options, sort, hide, delete. */
export function usePropertyMenu(db: DatabaseInfo | null, reload: () => void, view?: { sortBy: (pid: string, dir: "asc" | "desc") => void; hide: (pid: string) => void; filterBy: (pid: string) => void }) {
  const s = useStore();
  const { t } = useI18n();
  const menu = useAnchor();
  const [items, setItems] = useState<MenuEntry[]>([]);
  const [optionsFor, setOptionsFor] = useState<{ p: Property; anchor: Anchor } | null>(null);
  const [configFor, setConfigFor] = useState<Property | null>(null);

  const changeType = async (p: Property, type: PropertyType) => {
    try {
      const preview = await api.databases.changePropertyType(p.id, type, false);
      if (preview.lost > 0) {
        const ok = await s.confirm({
          title: t("Change property type?"),
          message: t("{lost} of {total} values cannot be converted and will be removed (e.g. {samples}).", { lost: preview.lost, total: preview.total, samples: preview.lostSamples.slice(0, 3).join(", ") }),
          confirmLabel: t("Change and remove values"),
          danger: true,
        });
        if (!ok) return;
      }
      await api.databases.changePropertyType(p.id, type, true);
      reload();
    } catch (e) {
      s.fail(e);
    }
  };

  const open = (anchor: Anchor, p: Property) => {
    const isTitle = p.type === "title";
    setItems([
      {
        label: t("Rename"), icon: <Icon name="page" />,
        onSelect: async () => {
          const name = await s.prompt({ title: t("Rename property"), defaultValue: p.name });
          if (name?.trim()) api.databases.updateProperty(p.id, { name }).then(reload).catch(s.fail);
        },
      },
      ...(p.config.options ? [{ label: t("Edit options"), icon: <span>◉</span>, onSelect: () => setOptionsFor({ p, anchor }) } as MenuEntry] : []),
      ...(["formula", "relation", "rollup"].includes(p.type)
        ? [{ label: t(p.type === "formula" ? "Edit formula" : p.type === "relation" ? "Relation settings" : "Rollup settings"), icon: <span>{typeIcon(p.type)}</span>, onSelect: () => setConfigFor(p) } as MenuEntry]
        : []),
      ...(view
        ? [
            { separator: true } as MenuEntry,
            { label: t("Sort ascending"), icon: <Icon name="sort" />, onSelect: () => view.sortBy(p.id, "asc") } as MenuEntry,
            { label: t("Sort descending"), icon: <Icon name="sort" />, onSelect: () => view.sortBy(p.id, "desc") } as MenuEntry,
            { label: t("Filter"), icon: <Icon name="filter" />, onSelect: () => view.filterBy(p.id) } as MenuEntry,
            ...(!isTitle ? [{ label: t("Hide in view"), icon: <Icon name="eye" />, onSelect: () => view.hide(p.id) } as MenuEntry] : []),
          ]
        : []),
      ...(!isTitle && !["created_time", "last_edited_time", "formula", "rollup"].includes(p.type)
        ? [
            { separator: true } as MenuEntry,
            { heading: t("Change type") } as MenuEntry,
            ...PROPERTY_TYPES.filter((x) => x.type !== p.type).map((x) => ({ label: t(x.label), icon: <span style={{ width: 16 }}>{x.icon}</span>, onSelect: () => changeType(p, x.type) })),
          ]
        : []),
      ...(!isTitle
        ? [
            { separator: true } as MenuEntry,
            {
              label: t("Delete property"), danger: true, icon: <Icon name="trash" />,
              onSelect: async () => {
                if (await s.confirm({ title: t("Delete property “{name}”?", { name: p.name }), message: t("Its values in every record are removed. This cannot be undone."), danger: true, confirmLabel: t("Delete") }))
                  api.databases.deleteProperty(p.id).then(reload).catch(s.fail);
              },
            } as MenuEntry,
          ]
        : []),
    ]);
    menu.open(anchor);
  };

  const element = (
    <>
      {menu.anchor && <Menu anchor={menu.anchor} items={items} onClose={menu.close} />}
      {configFor && db && <ComputedConfigDialog p={configFor} db={db} onClose={() => setConfigFor(null)} onSaved={() => (setConfigFor(null), reload())} />}
      {optionsFor && (
        <Popover anchor={optionsFor.anchor} onClose={() => setOptionsFor(null)} width={300}>
          <OptionsEditor p={optionsFor.p} onSaved={() => { reload(); }} t={t} />
        </Popover>
      )}
    </>
  );
  return { open, element };
}

const FORMULA_HELP = [
  ['prop("Name")', "value of a property"],
  ["+ - * / %", "math (+ also joins text)"],
  ["== != > < >= <=, and, or, not", "comparisons and logic"],
  ['if(cond, a, b)', "choose a value"],
  ['concat(a, b, …), upper(), lower(), length(), contains(text, part), replace(t, a, b)', "text"],
  ["round(n, digits), floor, ceil, abs, min, max, sum, average, toNumber, toText", "numbers"],
  ['today(), dateAdd(d, 3, "days"), dateSubtract, dateBetween(a, b, "days"), formatDate(d, "DD/MM/YYYY"), year, month, day', "dates"],
  ["empty(x)", "true when blank"],
];
const ROLLUP_FUNCTIONS = ["count", "count_values", "sum", "average", "min", "max", "percent_checked", "earliest", "latest", "show_original"];

/** Settings for computed and linked properties: formula expression, relation target, rollup wiring. */
function ComputedConfigDialog({ p, db, onClose, onSaved }: { p: Property; db: DatabaseInfo; onClose: () => void; onSaved: () => void }) {
  const s = useStore();
  const { t } = useI18n();
  const [cfg, setCfg] = useState<Record<string, unknown>>({ ...p.config });
  const [dbs, setDbs] = useState<{ id: string; title: string }[]>([]);
  const [targetProps, setTargetProps] = useState<Property[]>([]);
  const [error, setError] = useState("");
  useEffect(() => {
    if (p.type === "relation") api.pages.ofKind("database").then((x) => setDbs(x.map((d) => ({ id: d.id, title: d.title })))).catch(s.fail);
  }, [p.type, s.fail]);
  const relProp = db.properties.find((x) => x.id === cfg.relationPropertyId);
  useEffect(() => {
    const target = relProp?.config.databaseId as string | undefined;
    if (p.type === "rollup" && target) api.databases.get(target).then((d) => setTargetProps(d.properties)).catch(() => setTargetProps([]));
  }, [p.type, relProp]);
  const save = () => api.databases.updateProperty(p.id, { config: cfg }).then(onSaved).catch((e) => setError(String((e as { message?: string }).message ?? e)));
  return (
    <Modal title={`${typeIcon(p.type)} ${p.name}`} onClose={onClose} wide footer={<><button className="btn" onClick={onClose}>{t("Cancel")}</button><button className="btn primary" onClick={save}>{t("Save")}</button></>}>
      {p.type === "formula" && (
        <>
          <div className="field">
            <label>{t("Formula")}</label>
            <textarea className="textarea mono" rows={4} autoFocus spellCheck={false} value={String(cfg.expression ?? "")} onChange={(e) => setCfg({ ...cfg, expression: e.target.value })}
              placeholder={'prop("Price") * prop("Qty")'} />
          </div>
          <div className="small-text muted" style={{ marginBottom: 6 }}>{t("Properties")}: {db.properties.filter((x) => x.id !== p.id).map((x) => (
            <button key={x.id} className="chip" style={{ margin: 2, border: 0 }} onClick={() => setCfg({ ...cfg, expression: `${cfg.expression ?? ""}prop("${x.name}")` })}>{x.name}</button>
          ))}</div>
          <table className="plain small-text">
            <tbody>{FORMULA_HELP.map(([code, what]) => <tr key={code}><td className="mono">{code}</td><td className="muted">{t(what)}</td></tr>)}</tbody>
          </table>
        </>
      )}
      {p.type === "relation" && (
        <div className="field">
          <label>{t("Related database")}</label>
          <select className="select" value={String(cfg.databaseId ?? "")} onChange={(e) => setCfg({ ...cfg, databaseId: e.target.value })}>
            <option value="">{t("Choose…")}</option>
            {dbs.map((d) => <option key={d.id} value={d.id}>{d.title || t("Untitled")}{d.id === db.id ? ` (${t("this database")})` : ""}</option>)}
          </select>
        </div>
      )}
      {p.type === "rollup" && (
        <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr 1fr", gap: "0 12px" }}>
          <div className="field">
            <label>{t("Relation")}</label>
            <select className="select" value={String(cfg.relationPropertyId ?? "")} onChange={(e) => setCfg({ ...cfg, relationPropertyId: e.target.value, targetPropertyId: "" })}>
              <option value="">{t("Choose…")}</option>
              {db.properties.filter((x) => x.type === "relation").map((x) => <option key={x.id} value={x.id}>{x.name}</option>)}
            </select>
          </div>
          <div className="field">
            <label>{t("Property")}</label>
            <select className="select" value={String(cfg.targetPropertyId ?? "")} onChange={(e) => setCfg({ ...cfg, targetPropertyId: e.target.value })}>
              <option value="">—</option>
              {targetProps.map((x) => <option key={x.id} value={x.id}>{x.name}</option>)}
            </select>
          </div>
          <div className="field">
            <label>{t("Calculate")}</label>
            <select className="select" value={String(cfg.function ?? "count")} onChange={(e) => setCfg({ ...cfg, function: e.target.value })}>
              {ROLLUP_FUNCTIONS.map((f) => <option key={f} value={f}>{t(f)}</option>)}
            </select>
          </div>
        </div>
      )}
      {error && <div className="cb-error" role="alert">{error}</div>}
    </Modal>
  );
}

function OptionsEditor({ p, onSaved, t }: { p: Property; onSaved: () => void; t: (s: string) => string }) {
  const s = useStore();
  const [opts, setOpts] = useState<SelectOption[]>(p.config.options ?? []);
  const save = (next: SelectOption[]) => {
    setOpts(next);
    api.databases.updateProperty(p.id, { config: { ...p.config, options: next } }).then(onSaved).catch(s.fail);
  };
  return (
    <div style={{ padding: 4 }}>
      <div className="menu-label">{t("Options")}</div>
      {opts.map((o, i) => (
        <div key={o.id} className="row" style={{ marginBottom: 4 }}>
          <select className="select" style={{ width: 84 }} value={o.color} onChange={(e) => save(opts.map((x, j) => (j === i ? { ...x, color: e.target.value } : x)))} aria-label={t("Color")}>
            {COLORS.map((c) => <option key={c} value={c}>{c}</option>)}
          </select>
          <input className="input" defaultValue={o.name} onBlur={(e) => e.target.value.trim() && e.target.value !== o.name && save(opts.map((x, j) => (j === i ? { ...x, name: e.target.value.trim() } : x)))} />
          {p.type === "status" && (
            <select className="select" style={{ width: 96 }} value={o.group ?? "todo"} onChange={(e) => save(opts.map((x, j) => (j === i ? { ...x, group: e.target.value as SelectOption["group"] } : x)))} aria-label={t("Group")}>
              <option value="todo">{t("To do")}</option>
              <option value="in_progress">{t("In progress")}</option>
              <option value="done">{t("Done")}</option>
            </select>
          )}
          <button className="icon-btn small" aria-label={t("Move up")} disabled={i === 0} onClick={() => { const n = [...opts]; [n[i - 1], n[i]] = [n[i], n[i - 1]]; save(n); }}>↑</button>
          <button className="icon-btn small" aria-label={t("Delete option")} onClick={() => save(opts.filter((x) => x.id !== o.id))}><Icon name="close" size={12} /></button>
        </div>
      ))}
      <button className="btn small" onClick={() => save([...opts, { id: crypto.randomUUID(), name: t("New option"), color: COLORS[opts.length % COLORS.length] }])}>＋ {t("Add option")}</button>
    </div>
  );
}

export function AddPropertyButton({ db, onAdded, label }: { db: DatabaseInfo; onAdded: () => void; label?: string }) {
  const s = useStore();
  const { t } = useI18n();
  const menu = useAnchor();
  return (
    <>
      <button className="btn small ghost" onClick={menu.toggle} aria-label={t("Add property")}>＋ {label ?? ""}</button>
      {menu.anchor && (
        <Menu
          anchor={menu.anchor}
          onClose={menu.close}
          items={[
            { heading: t("Add property") },
            ...PROPERTY_TYPES.map((x) => ({
              label: t(x.label),
              icon: <span style={{ width: 16 }}>{x.icon}</span>,
              onSelect: async () => {
                const name = await s.prompt({ title: t("Property name"), defaultValue: t(x.label) });
                if (name?.trim()) api.databases.addProperty(db.id, name, x.type).then(onAdded).catch(s.fail);
              },
            })),
          ]}
        />
      )}
    </>
  );
}
