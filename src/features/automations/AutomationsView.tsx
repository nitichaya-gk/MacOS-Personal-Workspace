import { useCallback, useEffect, useState } from "react";
import { useStore } from "../../app/providers/store";
import { useI18n } from "../../app/providers/i18n";
import { api, rpc } from "../../services/desktop-bridge/api";
import type { DatabaseInfo, FilterGroup, PageMeta } from "../../shared/types";
import { Icon } from "../../components/ui/Icon";
import { EmptyState, Modal } from "../../components/ui/Modal";
import { FilterEditor } from "../databases/filters";

type Obj = Record<string, unknown>;
interface Rule {
  id: string;
  name: string;
  enabled: boolean;
  config: { trigger: Obj; condition?: Obj | null; actions: Obj[] };
  lastRun: { status: string; detail: string | null; at: string } | null;
}
interface Run {
  id: string;
  automationId: string;
  automationName: string;
  status: string;
  detail: string | null;
  at: string;
}

const auto = {
  list: () => rpc<Rule[]>("automations.list"),
  create: (name: string, config: Obj) => rpc<Rule>("automations.create", { name, config }),
  update: (id: string, patch: Obj) => rpc<Rule>("automations.update", { id, patch }),
  remove: (id: string) => rpc("automations.delete", { id }),
  runs: (id?: string) => rpc<Run[]>("automations.runs", { id, limit: 100 }),
  retry: (runId: string) => rpc<Run>("automations.retry", { runId }),
};

const TRIGGERS = [
  ["record_created", "A record is added to a database"],
  ["record_updated", "A record property changes"],
  ["task_created", "A task is created"],
  ["task_status_changed", "A task's status changes"],
  ["schedule", "Every day at a time"],
] as const;
const ACTIONS = [
  ["set_property", "Set a property of the record"],
  ["create_task", "Create a task"],
  ["set_task_status", "Set the task's status"],
  ["create_page_from_template", "Create a page from a template"],
  ["notify", "Show a macOS notification"],
] as const;

function describe(r: Rule, t: (s: string) => string) {
  const trig = TRIGGERS.find(([k]) => k === r.config.trigger.type)?.[1] ?? "";
  const acts = r.config.actions.map((a) => t(ACTIONS.find(([k]) => k === a.type)?.[1] ?? String(a.type))).join(" → ");
  return `${t(trig)} → ${acts}`;
}

export function AutomationsView() {
  const s = useStore();
  const { t, locale } = useI18n();
  const [rules, setRules] = useState<Rule[] | null>(null);
  const [runs, setRuns] = useState<Run[]>([]);
  const [editing, setEditing] = useState<Rule | "new" | null>(null);
  const load = useCallback(() => {
    auto.list().then(setRules).catch(s.fail);
    auto.runs().then(setRuns).catch(() => {});
  }, [s.fail]);
  useEffect(() => {
    load();
  }, [load]);
  return (
    <div className="content">
      <div className="page" style={{ maxWidth: 980 }}>
        <div className="row" style={{ marginTop: 16 }}>
          <h1 style={{ margin: 0, fontSize: 30 }}>⚡ {t("Automations")}</h1>
          <span className="spacer" />
          <button className="btn primary" onClick={() => setEditing("new")}>＋ {t("New automation")}</button>
        </div>
        <p className="muted small-text">{t("When something happens (trigger) and a condition holds, run actions. Each event runs a rule at most once, a rule never re-triggers itself on the same item, and failed runs roll back. Rules run while the app is open.")}</p>
        {rules === null ? <div className="loading">…</div> : rules.length === 0 ? (
          <EmptyState icon="⚡" title={t("No automations yet")} hint={t("e.g. when a record's Status becomes Done, create a follow-up task.")}><button className="btn primary" onClick={() => setEditing("new")}>＋ {t("New automation")}</button></EmptyState>
        ) : rules.map((r) => (
          <div key={r.id} className="card-box" style={{ padding: "10px 14px" }}>
            <div className="row">
              <label className="row" title={r.enabled ? t("On") : t("Off")}>
                <input type="checkbox" checked={r.enabled} onChange={(e) => auto.update(r.id, { enabled: e.target.checked }).then(load).catch(s.fail)} aria-label={t("Enabled")} />
              </label>
              <strong style={{ flex: 1 }}>{r.name}</strong>
              {r.lastRun && <span className={`chip ${r.lastRun.status === "ok" ? "c-green" : r.lastRun.status === "error" ? "c-red" : "c-yellow"}`} title={r.lastRun.detail ?? ""}>{r.lastRun.status === "ok" ? "✓" : r.lastRun.status === "error" ? "✕" : "–"} {t(r.lastRun.status)} · {new Date(r.lastRun.at).toLocaleString(locale)}</span>}
              <button className="btn small" onClick={() => setEditing(r)}>{t("Edit")}</button>
              <button className="icon-btn small" aria-label={t("Delete")} onClick={async () => { if (await s.confirm({ title: t("Delete this automation?"), danger: true, confirmLabel: t("Delete") })) auto.remove(r.id).then(load).catch(s.fail); }}><Icon name="trash" size={13} /></button>
            </div>
            <div className="small-text muted" style={{ paddingLeft: 26 }}>{describe(r, t)}</div>
          </div>
        ))}
        <div className="h-section">{t("Run history")}</div>
        {runs.length === 0 ? <div className="muted small-text">{t("No runs yet.")}</div> : (
          <table className="plain">
            <thead><tr><th>{t("When")}</th><th>{t("Automation")}</th><th>{t("Result")}</th><th>{t("Details")}</th><th /></tr></thead>
            <tbody>
              {runs.map((r) => (
                <tr key={r.id}>
                  <td className="muted">{new Date(r.at).toLocaleString(locale)}</td>
                  <td>{r.automationName}</td>
                  <td>{r.status === "ok" ? "✓ ok" : r.status === "error" ? `✕ ${t("error")}` : `– ${t("skipped")}`}</td>
                  <td className="small-text" style={{ maxWidth: 360 }}>{r.detail}</td>
                  <td>{r.status !== "ok" && <button className="btn small" onClick={() => auto.retry(r.id).then(load).catch(s.fail)}>{t("Retry")}</button>}</td>
                </tr>
              ))}
            </tbody>
          </table>
        )}
      </div>
      {editing && <RuleEditor rule={editing === "new" ? null : editing} onClose={() => setEditing(null)} onSaved={() => (setEditing(null), load())} />}
    </div>
  );
}

function RuleEditor({ rule, onClose, onSaved }: { rule: Rule | null; onClose: () => void; onSaved: () => void }) {
  const s = useStore();
  const { t } = useI18n();
  const [name, setName] = useState(rule?.name ?? "");
  const [trigger, setTrigger] = useState<Obj>(rule?.config.trigger ?? { type: "record_created" });
  const [condition, setCondition] = useState<Obj | null>(rule?.config.condition ?? null);
  const [actions, setActions] = useState<Obj[]>(rule?.config.actions ?? [{ type: "create_task", title: "Follow up: {{title}}", dueInDays: 1 }]);
  const [dbs, setDbs] = useState<PageMeta[]>([]);
  const [db, setDb] = useState<DatabaseInfo | null>(null);
  const [templates, setTemplates] = useState<PageMeta[]>([]);
  const [showFilter, setShowFilter] = useState(false);
  const type = String(trigger.type);
  const isRecord = type.startsWith("record_");
  const isTask = type.startsWith("task_") || !!trigger.forEach;
  useEffect(() => {
    api.pages.ofKind("database").then(setDbs).catch(() => {});
    api.templates.list().then(setTemplates).catch(() => {});
  }, []);
  useEffect(() => {
    if (isRecord && trigger.databaseId) api.databases.get(String(trigger.databaseId)).then(setDb).catch(() => setDb(null));
    else setDb(null);
  }, [isRecord, trigger.databaseId]);
  const setAction = (i: number, patch: Obj) => setActions(actions.map((a, j) => (j === i ? { ...a, ...patch } : a)));
  const save = () => {
    const config = { trigger, condition, actions };
    (rule ? auto.update(rule.id, { name, config }) : auto.create(name, config)).then(onSaved).catch(s.fail);
  };
  const settable = db?.properties.filter((p) => !["title", "created_time", "last_edited_time", "formula", "rollup", "files", "relation"].includes(p.type)) ?? [];

  return (
    <Modal title={rule ? t("Edit automation") : t("New automation")} wide onClose={onClose} footer={<><button className="btn" onClick={onClose}>{t("Cancel")}</button><button className="btn primary" onClick={save}>{t("Save")}</button></>}>
      <div className="field"><label>{t("Name")}</label><input className="input" autoFocus value={name} placeholder={t("e.g. Follow up finished leads")} onChange={(e) => setName(e.target.value)} /></div>

      <div className="h-section" style={{ marginTop: 4 }}>1 · {t("When")}</div>
      <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: "0 12px" }}>
        <div className="field">
          <select className="select" value={type} onChange={(e) => (setTrigger({ type: e.target.value }), setCondition(null))}>
            {TRIGGERS.map(([k, label]) => <option key={k} value={k}>{t(label)}</option>)}
          </select>
        </div>
        {isRecord && (
          <div className="field">
            <select className="select" value={String(trigger.databaseId ?? "")} onChange={(e) => setTrigger({ ...trigger, databaseId: e.target.value })} aria-label={t("Database")}>
              <option value="">{t("Choose a database…")}</option>
              {dbs.map((d) => <option key={d.id} value={d.id}>{d.title || t("Untitled")}</option>)}
            </select>
          </div>
        )}
        {type === "record_updated" && db && (
          <div className="field">
            <select className="select" value={String(trigger.propertyId ?? "")} onChange={(e) => setTrigger({ ...trigger, propertyId: e.target.value })} aria-label={t("Property")}>
              <option value="">{t("Any property")}</option>
              {db.properties.map((p) => <option key={p.id} value={p.id}>{p.name}</option>)}
            </select>
          </div>
        )}
        {type === "task_status_changed" && (
          <div className="field">
            <select className="select" value={String(trigger.toStatus ?? "")} onChange={(e) => setTrigger({ ...trigger, toStatus: e.target.value })} aria-label={t("Status")}>
              <option value="">{t("Any change")}</option><option value="todo">{t("To do")}</option><option value="in_progress">{t("In progress")}</option><option value="done">{t("Done")}</option>
            </select>
          </div>
        )}
        {type === "schedule" && (
          <>
            <div className="field"><input className="input" type="time" value={String(trigger.time ?? "08:00")} onChange={(e) => setTrigger({ ...trigger, time: e.target.value })} aria-label={t("Time")} /></div>
            <div className="field">
              <select className="select" value={String(trigger.forEach ?? "")} onChange={(e) => setTrigger({ ...trigger, forEach: e.target.value || undefined })} aria-label={t("For each")}>
                <option value="">{t("Once")}</option><option value="tasks_due_today">{t("For each task due today")}</option><option value="tasks_overdue">{t("For each overdue task")}</option>
              </select>
            </div>
          </>
        )}
      </div>

      <div className="h-section">2 · {t("Only if")} <span className="faint">({t("optional")})</span></div>
      {isRecord && db ? (
        <>
          <button className="btn small" onClick={() => setShowFilter(!showFilter)}>{condition?.filter ? `✓ ${t("Condition set")}` : t("Add condition")}</button>
          {showFilter && <FilterEditor db={db} config={{ filter: condition?.filter as FilterGroup | undefined }} onChange={(f) => setCondition(f.conditions.length ? { filter: f } : null)} />}
        </>
      ) : isTask ? (
        <div className="row wrap">
          <select className="select" style={{ width: 160 }} value={String(condition?.status ?? "")} onChange={(e) => setCondition({ ...(condition ?? {}), status: e.target.value })} aria-label={t("Status")}>
            <option value="">{t("Any status")}</option><option value="todo">{t("To do")}</option><option value="in_progress">{t("In progress")}</option><option value="done">{t("Done")}</option>
          </select>
          <select className="select" style={{ width: 160 }} value={String(condition?.priority ?? "")} onChange={(e) => setCondition({ ...(condition ?? {}), priority: e.target.value })} aria-label={t("Priority")}>
            <option value="">{t("Any priority")}</option>{["low", "medium", "high", "urgent"].map((p) => <option key={p} value={p}>{t(p[0].toUpperCase() + p.slice(1))}</option>)}
          </select>
          <label className="row small-text"><input type="checkbox" checked={!!condition?.dueToday} onChange={(e) => setCondition({ ...(condition ?? {}), dueToday: e.target.checked })} /> {t("Due today")}</label>
        </div>
      ) : <div className="small-text faint">{t("No conditions for this trigger.")}</div>}

      <div className="h-section">3 · {t("Then")}</div>
      {actions.map((a, i) => (
        <div key={i} className="card-box" style={{ padding: "8px 12px", marginBottom: 8 }}>
          <div className="row">
            <select className="select" value={String(a.type)} onChange={(e) => setAction(i, { type: e.target.value })} aria-label={t("Action")}>
              {ACTIONS.filter(([k]) => (k !== "set_property" || isRecord) && (k !== "set_task_status" || isTask)).map(([k, label]) => <option key={k} value={k}>{t(label)}</option>)}
            </select>
            <button className="icon-btn small" aria-label={t("Remove")} onClick={() => setActions(actions.filter((_, j) => j !== i))}><Icon name="close" size={12} /></button>
          </div>
          <div className="row wrap" style={{ marginTop: 6 }}>
            {a.type === "set_property" && (
              <>
                <select className="select" style={{ width: 180 }} value={String(a.propertyId ?? "")} onChange={(e) => setAction(i, { propertyId: e.target.value, value: null })} aria-label={t("Property")}>
                  <option value="">{t("Property…")}</option>{settable.map((p) => <option key={p.id} value={p.id}>{p.name}</option>)}
                </select>
                {(() => {
                  const p = settable.find((x) => x.id === a.propertyId);
                  if (!p) return null;
                  if (p.type === "select" || p.type === "status") return <select className="select" style={{ width: 180 }} value={String(a.value ?? "")} onChange={(e) => setAction(i, { value: e.target.value })}><option value="">—</option>{(p.config.options ?? []).map((o) => <option key={o.id} value={o.id}>{o.name}</option>)}</select>;
                  if (p.type === "checkbox") return <label className="row small-text"><input type="checkbox" checked={!!a.value} onChange={(e) => setAction(i, { value: e.target.checked })} /> {t("Checked")}</label>;
                  if (p.type === "date") return <input className="input" type="date" style={{ width: 160 }} value={String((a.value as { start?: string })?.start ?? "")} onChange={(e) => setAction(i, { value: { start: e.target.value } })} />;
                  return <input className="input" style={{ width: 220 }} value={String(a.value ?? "")} onChange={(e) => setAction(i, { value: p.type === "number" ? Number(e.target.value) : e.target.value })} />;
                })()}
              </>
            )}
            {a.type === "create_task" && (
              <>
                <input className="input" style={{ flex: 1 }} value={String(a.title ?? "")} onChange={(e) => setAction(i, { title: e.target.value })} aria-label={t("Task name")} />
                <label className="row small-text">{t("due in")} <input className="input" type="number" style={{ width: 64 }} value={String(a.dueInDays ?? "")} onChange={(e) => setAction(i, { dueInDays: e.target.value === "" ? null : Number(e.target.value) })} /> {t("days")}</label>
              </>
            )}
            {a.type === "set_task_status" && (
              <select className="select" style={{ width: 160 }} value={String(a.status ?? "done")} onChange={(e) => setAction(i, { status: e.target.value })}>
                <option value="todo">{t("To do")}</option><option value="in_progress">{t("In progress")}</option><option value="done">{t("Done")}</option>
              </select>
            )}
            {a.type === "create_page_from_template" && (
              <>
                <select className="select" style={{ width: 200 }} value={String(a.templateId ?? "")} onChange={(e) => setAction(i, { templateId: e.target.value })}>
                  <option value="">{t("Template…")}</option>{templates.map((tp) => <option key={tp.id} value={tp.id}>{tp.title}</option>)}
                </select>
                <input className="input" style={{ flex: 1 }} placeholder={t("Page title (optional)")} value={String(a.title ?? "")} onChange={(e) => setAction(i, { title: e.target.value })} />
              </>
            )}
            {a.type === "notify" && (
              <>
                <input className="input" style={{ flex: 1 }} placeholder={t("Title")} value={String(a.title ?? "")} onChange={(e) => setAction(i, { title: e.target.value })} />
                <input className="input" style={{ flex: 2 }} placeholder={t("Message")} value={String(a.body ?? "")} onChange={(e) => setAction(i, { body: e.target.value })} />
              </>
            )}
          </div>
        </div>
      ))}
      <button className="btn small" onClick={() => setActions([...actions, { type: "notify", title: "{{title}}" }])}>＋ {t("Add action")}</button>
      <p className="small-text faint">{t("Use {{title}} for the record or task name and {{date}} for the scheduled date.")}</p>
    </Modal>
  );
}
