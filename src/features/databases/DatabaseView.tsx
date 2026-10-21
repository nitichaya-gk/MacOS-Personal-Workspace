import { useCallback, useEffect, useMemo, useState } from "react";
import { save } from "@tauri-apps/plugin-dialog";
import { useStore } from "../../app/providers/store";
import { useI18n } from "../../app/providers/i18n";
import { api, errorMessage } from "../../services/desktop-bridge/api";
import type { DatabaseInfo, DbRecord, FilterGroup, SortSpec, View, ViewConfig, ViewType } from "../../shared/types";
import { debounce, fileSafeName } from "../../shared/utils";
import { Icon } from "../../components/ui/Icon";
import { Menu, Popover, useAnchor, type Anchor } from "../../components/ui/Popover";
import { usePropertyMenu } from "./cells";
import { countConditions, FilterEditor, PropertiesEditor, SortEditor } from "./filters";
import { BoardView, CalendarView, ListView, TableView, type ViewProps } from "./views";

const VIEW_ICONS: Record<ViewType, string> = { table: "table", board: "board", calendar: "calendar", list: "list" };
const PAGE = 200;

export function DatabaseView({ id, embedded, viewId, onViewChange }: { id: string; embedded?: boolean; viewId?: string; onViewChange?: (viewId: string) => void }) {
  const s = useStore();
  const { t } = useI18n();
  const [db, setDb] = useState<DatabaseInfo | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [activeId, setActiveId] = useState<string | undefined>(viewId);
  const [records, setRecords] = useState<DbRecord[]>([]);
  const [total, setTotal] = useState(0);
  const [limit, setLimit] = useState(PAGE);
  const [search, setSearch] = useState("");
  const [panel, setPanel] = useState<{ kind: "filter" | "sort" | "props"; anchor: Anchor; propertyId?: string } | null>(null);
  const newMenu = useAnchor();
  const moreMenu = useAnchor();
  const addView = useAnchor();

  const loadDb = useCallback(() => api.databases.get(id).then((d) => (setDb(d), setError(null))).catch((e) => setError(errorMessage(e))), [id]);
  useEffect(() => {
    loadDb();
  }, [loadDb]);

  const view: View | undefined = db?.views.find((v) => v.id === activeId) ?? db?.views[0];
  const loadRecords = useCallback(() => {
    if (!view) return;
    const lim = view.type === "table" || view.type === "list" ? limit : 2000;
    api.databases
      .query(id, { viewId: view.id, search: search || undefined, limit: lim })
      .then((r) => (setRecords(r.records), setTotal(r.total)))
      .catch(s.fail);
  }, [id, view?.id, view?.config, view?.type, search, limit, s.fail]);
  useEffect(() => {
    loadRecords();
  }, [loadRecords]);

  const reload = useCallback(() => {
    loadDb();
    loadRecords();
  }, [loadDb, loadRecords]);

  const saveConfig = useMemo(
    () => debounce((viewId: string, config: ViewConfig) => api.databases.updateView(viewId, { config }).then(loadDb).catch(s.fail), 250),
    [loadDb, s.fail],
  );
  const updateConfig = (patch: Partial<ViewConfig>) => {
    if (!view || !db) return;
    const config = { ...view.config, ...patch };
    // Optimistic: the view re-queries with the new config right away.
    setDb({ ...db, views: db.views.map((v) => (v.id === view.id ? { ...v, config } : v)) });
    saveConfig(view.id, config);
  };

  const propMenu = usePropertyMenu(reload, {
    sortBy: (pid, direction) => updateConfig({ sorts: [{ propertyId: pid, direction }, ...(view?.config.sorts ?? []).filter((x) => x.propertyId !== pid)] }),
    hide: (pid) => updateConfig({ hidden: [...(view?.config.hidden ?? []), pid] }),
    filterBy: (pid) => setPanel({ kind: "filter", anchor: document.querySelector(`[data-db="${id}"] .filter-btn`) as HTMLElement, propertyId: pid }),
  });

  if (error) return <div className="error-box">{t("This database could not be loaded.")} <span className="mono">{error}</span></div>;
  if (!db || !view) return <div className="loading">{t("Loading…")}</div>;

  const create = async (values: Record<string, unknown> = {}, title = "", templateId?: string) => {
    try {
      const r = await api.databases.createRecord(db.id, { title, values, templateId });
      setRecords((rs) => [...rs, r]);
      setTotal((n) => n + 1);
      if (templateId) s.navigate({ name: "page", id: r.id });
    } catch (e) {
      s.fail(e);
    }
  };
  const open = (r: DbRecord) => s.navigate({ name: "page", id: r.id });
  const replace = (r: DbRecord) => setRecords((rs) => rs.map((x) => (x.id === r.id ? r : x)));

  const props: ViewProps = {
    db, view, records, total, reload, replace, open,
    create: (values, title) => create(values, title),
    loadMore: () => setLimit((l) => l + PAGE),
    openPropMenu: propMenu.open,
  };

  const exportAs = async (format: "csv" | "json") => {
    const dest = await save({ defaultPath: `${fileSafeName(db.title)}.${format}`, filters: [{ name: format.toUpperCase(), extensions: [format] }] });
    if (!dest) return;
    api.exports.database(db.id, format, dest).then((r) => s.toast(r.warnings.length ? `${t("Exported")} — ${r.warnings[0]}` : t("Exported"), { kind: "success" })).catch(s.fail);
  };

  const filters = countConditions(view.config.filter);
  const sorts = view.config.sorts?.length ?? 0;
  const selectView = (v: View) => {
    setActiveId(v.id);
    setLimit(PAGE);
    onViewChange?.(v.id);
  };

  return (
    <div data-db={id}>
      {embedded && (
        <div className="row" style={{ marginBottom: 4 }}>
          <button className="crumb" style={{ fontWeight: 600, fontSize: 15 }} onClick={() => s.navigate({ name: "page", id: db.id })}>{db.icon || "🗂️"} {db.title || t("Untitled")}</button>
        </div>
      )}
      <div className="db-tabs" role="tablist">
        {db.views.map((v) => (
          <button
            key={v.id}
            role="tab"
            aria-selected={v.id === view.id}
            className={`db-tab${v.id === view.id ? " active" : ""}`}
            onClick={() => selectView(v)}
            onDoubleClick={async () => {
              const name = await s.prompt({ title: t("Rename view"), defaultValue: v.name });
              if (name?.trim()) api.databases.updateView(v.id, { name }).then(loadDb).catch(s.fail);
            }}
          >
            <Icon name={VIEW_ICONS[v.type]} size={14} /> {v.name}
          </button>
        ))}
        <button className="icon-btn small" aria-label={t("Add view")} onClick={addView.toggle}><Icon name="plus" size={14} /></button>
        <span className="spacer" />
      </div>
      <div className="db-toolbar">
        <button className={`btn small ghost filter-btn${filters ? " active-filter" : ""}`} onClick={(e) => setPanel({ kind: "filter", anchor: e.currentTarget })}>
          <Icon name="filter" size={13} /> {t("Filter")}{filters ? ` · ${filters}` : ""}
        </button>
        <button className={`btn small ghost${sorts ? " active-filter" : ""}`} onClick={(e) => setPanel({ kind: "sort", anchor: e.currentTarget })}>
          <Icon name="sort" size={13} /> {t("Sort")}{sorts ? ` · ${sorts}` : ""}
        </button>
        <button className="btn small ghost" onClick={(e) => setPanel({ kind: "props", anchor: e.currentTarget })}><Icon name="eye" size={13} /> {t("Properties")}</button>
        {view.type === "board" && (
          <select className="select" style={{ width: 160, height: 24, fontSize: 12 }} value={view.config.groupBy ?? ""} aria-label={t("Group by")} onChange={(e) => updateConfig({ groupBy: e.target.value || null })}>
            <option value="">{t("Group by…")}</option>
            {db.properties.filter((p) => p.type === "select" || p.type === "status").map((p) => <option key={p.id} value={p.id}>{t("Group by {name}", { name: p.name })}</option>)}
          </select>
        )}
        {view.type === "calendar" && (
          <select className="select" style={{ width: 160, height: 24, fontSize: 12 }} value={view.config.dateProperty ?? ""} aria-label={t("Date property")} onChange={(e) => updateConfig({ dateProperty: e.target.value || null })}>
            {db.properties.filter((p) => p.type === "date").map((p) => <option key={p.id} value={p.id}>{p.name}</option>)}
          </select>
        )}
        <span className="spacer" />
        <input className="input" style={{ width: 170, height: 26 }} placeholder={t("Search in database")} value={search} onChange={(e) => setSearch(e.target.value)} aria-label={t("Search in database")} />
        <span className="small-text faint">{t("{n} records", { n: total })}</span>
        <div className="row" style={{ gap: 0 }}>
          <button className="btn small primary" style={{ borderTopRightRadius: 0, borderBottomRightRadius: 0 }} onClick={() => create()}>{t("New")}</button>
          <button className="btn small primary" style={{ borderTopLeftRadius: 0, borderBottomLeftRadius: 0, padding: "0 5px", borderLeft: "1px solid rgba(255,255,255,.3)" }} aria-label={t("New from template")} onClick={newMenu.toggle}>
            <Icon name="chevronDown" size={12} />
          </button>
        </div>
        <button className="icon-btn small" aria-label={t("Database options")} onClick={moreMenu.toggle}><Icon name="more" size={14} /></button>
      </div>

      {view.type === "table" && <TableView {...props} />}
      {view.type === "board" && <BoardView {...props} />}
      {view.type === "calendar" && <CalendarView {...props} />}
      {view.type === "list" && <ListView {...props} />}

      {propMenu.element}
      {panel && (
        <Popover anchor={panel.anchor} onClose={() => setPanel(null)}>
          {panel.kind === "filter" && <FilterEditor db={db} config={view.config} initialProperty={panel.propertyId} onChange={(f: FilterGroup) => updateConfig({ filter: f })} />}
          {panel.kind === "sort" && <SortEditor db={db} config={view.config} onChange={(x: SortSpec[]) => updateConfig({ sorts: x })} />}
          {panel.kind === "props" && <PropertiesEditor db={db} config={view.config} onChange={(hidden) => updateConfig({ hidden })} />}
        </Popover>
      )}
      {addView.anchor && (
        <Menu
          anchor={addView.anchor}
          onClose={addView.close}
          items={(["table", "board", "calendar", "list"] as ViewType[]).map((vt) => ({
            label: t(vt[0].toUpperCase() + vt.slice(1)),
            icon: <Icon name={VIEW_ICONS[vt]} />,
            onSelect: () => api.databases.addView(db.id, vt, t(vt[0].toUpperCase() + vt.slice(1))).then((v) => (loadDb(), selectView(v))).catch(s.fail),
          }))}
        />
      )}
      {newMenu.anchor && (
        <Menu
          anchor={newMenu.anchor}
          align="end"
          onClose={newMenu.close}
          items={[
            { heading: t("Templates") },
            ...db.templates.map((tp) => ({
              label: tp.title || t("Untitled"),
              icon: <span>{tp.icon || "🧩"}</span>,
              onSelect: () => create({}, "", tp.id),
            })),
            ...(db.templates.length === 0 ? [{ label: t("No templates yet"), disabled: true }] : []),
            { separator: true },
            ...db.templates.map((tp) => ({ label: t("Edit “{name}”", { name: tp.title || t("Untitled") }), icon: <Icon name="page" />, onSelect: () => s.navigate({ name: "page", id: tp.id }) })),
            {
              label: t("New template"),
              icon: <Icon name="plus" />,
              onSelect: async () => {
                const r = await api.databases.createRecord(db.id, { title: t("New template"), isTemplate: true }).catch((e) => (s.fail(e), null));
                if (r) s.navigate({ name: "page", id: r.id });
              },
            },
          ]}
        />
      )}
      {moreMenu.anchor && (
        <Menu
          anchor={moreMenu.anchor}
          align="end"
          onClose={moreMenu.close}
          items={[
            { label: t("Rename view"), icon: <Icon name="page" />, onSelect: async () => { const name = await s.prompt({ title: t("Rename view"), defaultValue: view.name }); if (name?.trim()) api.databases.updateView(view.id, { name }).then(loadDb).catch(s.fail); } },
            { heading: t("Change view type") },
            ...(["table", "board", "calendar", "list"] as ViewType[]).filter((vt) => vt !== view.type).map((vt) => ({
              label: t(vt[0].toUpperCase() + vt.slice(1)), icon: <Icon name={VIEW_ICONS[vt]} />,
              onSelect: () => api.databases.updateView(view.id, { type: vt }).then(loadDb).catch(s.fail),
            })),
            { separator: true },
            { label: t("Export CSV…"), icon: <Icon name="download" />, onSelect: () => exportAs("csv") },
            { label: t("Export JSON…"), icon: <Icon name="download" />, onSelect: () => exportAs("json") },
            { separator: true },
            {
              label: db.mcpAccess === "deny" ? t("Allow MCP clients to read") : t("Hide from MCP clients"),
              icon: <Icon name="mcp" />,
              onSelect: () => api.databases.setMcpAccess(db.id, db.mcpAccess === "deny" ? "inherit" : "deny").then(loadDb).catch(s.fail),
            },
            ...(db.views.length > 1
              ? [{ separator: true }, {
                  label: t("Delete view"), danger: true, icon: <Icon name="trash" />,
                  onSelect: async () => { if (await s.confirm({ title: t("Delete view “{name}”?", { name: view.name }), message: t("Records are not affected."), danger: true, confirmLabel: t("Delete") })) api.databases.deleteView(view.id).then(() => (setActiveId(undefined), loadDb())).catch(s.fail); },
                }]
              : []),
          ]}
        />
      )}
    </div>
  );
}
