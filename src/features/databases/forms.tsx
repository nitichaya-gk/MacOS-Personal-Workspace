import { useEffect, useState } from "react";
import { useStore } from "../../app/providers/store";
import { useI18n } from "../../app/providers/i18n";
import { api, rpc } from "../../services/desktop-bridge/api";
import type { DatabaseInfo, DbRecord, Property } from "../../shared/types";
import { Icon } from "../../components/ui/Icon";
import { EmptyState, Modal } from "../../components/ui/Modal";
import { typeIcon } from "./cells";

interface Field {
  propertyId: string;
  label: string;
  required: boolean;
  help: string;
}
interface FormConfig {
  description: string;
  fields: Field[];
  submitLabel: string;
  successMessage: string;
}
interface FormDef {
  id: string;
  databaseId: string;
  databaseTitle: string;
  name: string;
  config: FormConfig;
  properties: Property[];
}

export const formsApi = {
  list: (databaseId: string) => rpc<{ id: string; name: string }[]>("forms.list", { databaseId }),
  get: (id: string) => rpc<FormDef>("forms.get", { id }),
  create: (databaseId: string, name: string) => rpc<FormDef>("forms.create", { databaseId, name }),
  update: (id: string, patch: { name?: string; config?: FormConfig }) => rpc<FormDef>("forms.update", { id, patch }),
  remove: (id: string) => rpc("forms.delete", { id }),
  submit: (id: string, values: Record<string, unknown>) => rpc<{ ok: boolean; errors?: Record<string, string>; record?: DbRecord; message?: string }>("forms.submit", { id, values }),
};

const editable = (p: Property) => p.type === "title" || !["created_time", "last_edited_time", "formula", "rollup"].includes(p.type);

/** List, create and edit the forms of one database. */
export function FormsManager({ db, onClose }: { db: DatabaseInfo; onClose: () => void }) {
  const s = useStore();
  const { t } = useI18n();
  const [forms, setForms] = useState<{ id: string; name: string }[]>([]);
  const [editing, setEditing] = useState<FormDef | null>(null);
  const load = () => formsApi.list(db.id).then(setForms).catch(s.fail);
  useEffect(() => {
    load();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [db.id]);

  if (editing) return <FormEditor form={editing} onClose={() => (setEditing(null), load())} />;
  return (
    <Modal title={`${t("Forms")} · ${db.title || t("Untitled")}`} onClose={onClose}
      footer={<><button className="btn" onClick={onClose}>{t("Close")}</button><button className="btn primary" onClick={async () => {
        const name = await s.prompt({ title: t("Form name"), defaultValue: `${db.title} ${t("form")}` });
        if (name !== null) formsApi.create(db.id, name).then(setEditing).catch(s.fail);
      }}>＋ {t("New form")}</button></>}>
      <p className="small-text muted" style={{ marginTop: 0 }}>{t("Forms are filled in inside this app. Each valid submission adds one record; required fields and value types are checked first.")}</p>
      {forms.length === 0 ? <EmptyState icon="📝" title={t("No forms yet")} /> : forms.map((f) => (
        <div key={f.id} className="list-row">
          <span>📝</span><span className="t">{f.name}</span>
          <button className="btn small primary" onClick={() => (onClose(), s.navigate({ name: "form", id: f.id }))}>{t("Fill in")}</button>
          <button className="btn small" onClick={() => formsApi.get(f.id).then(setEditing).catch(s.fail)}>{t("Edit")}</button>
          <button className="icon-btn small" aria-label={t("Delete")} onClick={async () => { if (await s.confirm({ title: t("Delete this form?"), message: t("Records it created stay."), danger: true, confirmLabel: t("Delete") })) formsApi.remove(f.id).then(load).catch(s.fail); }}><Icon name="trash" size={13} /></button>
        </div>
      ))}
    </Modal>
  );
}

function FormEditor({ form, onClose }: { form: FormDef; onClose: () => void }) {
  const s = useStore();
  const { t } = useI18n();
  const [name, setName] = useState(form.name);
  const [cfg, setCfg] = useState<FormConfig>(form.config);
  const used = new Set(cfg.fields.map((f) => f.propertyId));
  const setField = (i: number, patch: Partial<Field>) => setCfg({ ...cfg, fields: cfg.fields.map((f, j) => (j === i ? { ...f, ...patch } : f)) });
  const move = (i: number, d: number) => {
    const f = [...cfg.fields];
    const j = i + d;
    if (j < 0 || j >= f.length) return;
    [f[i], f[j]] = [f[j], f[i]];
    setCfg({ ...cfg, fields: f });
  };
  const save = () => formsApi.update(form.id, { name, config: cfg }).then(() => (s.toast(t("Form saved"), { kind: "success" }), onClose())).catch(s.fail);
  return (
    <Modal title={t("Edit form")} wide onClose={onClose} footer={<><button className="btn" onClick={onClose}>{t("Cancel")}</button><button className="btn primary" onClick={save}>{t("Save")}</button></>}>
      <div className="field"><label>{t("Name")}</label><input className="input" value={name} onChange={(e) => setName(e.target.value)} /></div>
      <div className="field"><label>{t("Description")}</label><textarea className="textarea" rows={2} value={cfg.description} onChange={(e) => setCfg({ ...cfg, description: e.target.value })} /></div>
      <div className="menu-label" style={{ paddingLeft: 0 }}>{t("Fields")}</div>
      {cfg.fields.map((f, i) => {
        const p = form.properties.find((x) => x.id === f.propertyId);
        return (
          <div key={f.propertyId} className="row" style={{ marginBottom: 6 }}>
            <span className="muted" style={{ width: 20 }}>{p ? typeIcon(p.type) : "?"}</span>
            <input className="input" style={{ flex: 2 }} value={f.label} onChange={(e) => setField(i, { label: e.target.value })} aria-label={t("Label")} />
            <input className="input" style={{ flex: 2 }} value={f.help} placeholder={t("Help text")} onChange={(e) => setField(i, { help: e.target.value })} />
            <label className="row small-text"><input type="checkbox" checked={f.required} onChange={(e) => setField(i, { required: e.target.checked })} /> {t("Required")}</label>
            <button className="icon-btn small" aria-label={t("Move up")} onClick={() => move(i, -1)}>↑</button>
            <button className="icon-btn small" aria-label={t("Move down")} onClick={() => move(i, 1)}>↓</button>
            <button className="icon-btn small" aria-label={t("Remove")} disabled={p?.type === "title"} onClick={() => setCfg({ ...cfg, fields: cfg.fields.filter((_, j) => j !== i) })}><Icon name="close" size={12} /></button>
          </div>
        );
      })}
      <select className="select" style={{ width: 260, marginBottom: 12 }} value="" onChange={(e) => {
        const p = form.properties.find((x) => x.id === e.target.value);
        if (p) setCfg({ ...cfg, fields: [...cfg.fields, { propertyId: p.id, label: p.name, required: false, help: "" }] });
      }} aria-label={t("Add field")}>
        <option value="">＋ {t("Add field…")}</option>
        {form.properties.filter((p) => editable(p) && !used.has(p.id)).map((p) => <option key={p.id} value={p.id}>{p.name}</option>)}
      </select>
      <div style={{ display: "grid", gridTemplateColumns: "1fr 2fr", gap: 12 }}>
        <div className="field"><label>{t("Button label")}</label><input className="input" value={cfg.submitLabel} onChange={(e) => setCfg({ ...cfg, submitLabel: e.target.value })} /></div>
        <div className="field"><label>{t("Message after submitting")}</label><input className="input" value={cfg.successMessage} onChange={(e) => setCfg({ ...cfg, successMessage: e.target.value })} /></div>
      </div>
    </Modal>
  );
}

function RelationPicker({ target, value, onChange }: { target: string; value: string[]; onChange: (v: string[]) => void }) {
  const [recs, setRecs] = useState<DbRecord[]>([]);
  useEffect(() => {
    if (target) api.databases.query(target, { limit: 300 }).then((r) => setRecs(r.records)).catch(() => {});
  }, [target]);
  return (
    <select className="select" multiple size={Math.min(6, Math.max(3, recs.length))} style={{ height: "auto" }} value={value} onChange={(e) => onChange([...e.target.selectedOptions].map((o) => o.value))}>
      {recs.map((r) => <option key={r.id} value={r.id}>{r.title}</option>)}
    </select>
  );
}

/** Filling in a form (route `form/<id>`). */
export function FormFillView({ id }: { id: string }) {
  const s = useStore();
  const { t } = useI18n();
  const [form, setForm] = useState<FormDef | null>(null);
  const [values, setValues] = useState<Record<string, unknown>>({});
  const [errors, setErrors] = useState<Record<string, string>>({});
  const [done, setDone] = useState<string | null>(null);
  useEffect(() => {
    formsApi.get(id).then(setForm).catch(s.fail);
  }, [id, s.fail]);
  if (!form) return <div className="content"><div className="loading">…</div></div>;
  const set = (pid: string, v: unknown) => setValues({ ...values, [pid]: v });
  const submit = async () => {
    try {
      const r = await formsApi.submit(form.id, values);
      if (!r.ok) {
        setErrors(r.errors ?? {});
        s.toast(t("Please fix the highlighted fields."), { kind: "error" });
        return;
      }
      setErrors({});
      setValues({});
      setDone(r.message ?? t("Saved"));
    } catch (e) {
      s.fail(e);
    }
  };
  return (
    <div className="content">
      <div className="page" style={{ maxWidth: 640 }}>
        <div className="small-text muted" style={{ marginTop: 16 }}>
          📝 <button className="crumb muted" onClick={() => s.navigate({ name: "page", id: form.databaseId })}>{form.databaseTitle}</button>
        </div>
        <h1 style={{ margin: "6px 0", fontSize: 28 }}>{form.name}</h1>
        {form.config.description && <p className="muted" style={{ whiteSpace: "pre-wrap" }}>{form.config.description}</p>}
        {done ? (
          <div className="card-box">
            <p>✅ {done}</p>
            <div className="row">
              <button className="btn primary" onClick={() => setDone(null)}>{t("Submit another response")}</button>
              <button className="btn" onClick={() => s.navigate({ name: "page", id: form.databaseId })}>{t("Open database")}</button>
            </div>
          </div>
        ) : (
          <form onSubmit={(e) => (e.preventDefault(), submit())} noValidate>
            {form.config.fields.map((f) => {
              const p = form.properties.find((x) => x.id === f.propertyId);
              if (!p) return null;
              const v = values[p.id];
              const err = errors[p.id];
              const idAttr = `ff-${p.id}`;
              let input;
              switch (p.type) {
                case "select":
                case "status":
                  input = <select id={idAttr} className="select" value={String(v ?? "")} onChange={(e) => set(p.id, e.target.value || null)}><option value="">—</option>{(p.config.options ?? []).map((o) => <option key={o.id} value={o.id}>{o.name}</option>)}</select>;
                  break;
                case "multi_select":
                  input = <div className="row wrap">{(p.config.options ?? []).map((o) => {
                    const cur = (v as string[]) ?? [];
                    return <label key={o.id} className="row small-text"><input type="checkbox" checked={cur.includes(o.id)} onChange={(e) => set(p.id, e.target.checked ? [...cur, o.id] : cur.filter((x) => x !== o.id))} /> {o.name}</label>;
                  })}</div>;
                  break;
                case "checkbox":
                  input = <label className="row"><input id={idAttr} type="checkbox" checked={!!v} onChange={(e) => set(p.id, e.target.checked)} /> {f.label}</label>;
                  break;
                case "date":
                  input = <input id={idAttr} className="input" type="date" value={(v as { start?: string })?.start ?? ""} onChange={(e) => set(p.id, e.target.value ? { start: e.target.value } : null)} />;
                  break;
                case "number":
                  input = <input id={idAttr} className="input" inputMode="decimal" value={String(v ?? "")} onChange={(e) => set(p.id, e.target.value)} />;
                  break;
                case "relation":
                  input = <RelationPicker target={String(p.config.databaseId ?? "")} value={(v as string[]) ?? []} onChange={(x) => set(p.id, x)} />;
                  break;
                case "files":
                  input = (
                    <div>
                      {((v as { assetId: string; name: string }[]) ?? []).map((x) => <span key={x.assetId} className="chip">📎 {x.name}</span>)}
                      <input id={idAttr} type="file" multiple onChange={async (e) => {
                        const up = await Promise.all(Array.from(e.target.files ?? []).map((file) => api.assets.upload(file, form.databaseId))).catch((err) => (s.fail(err), []));
                        set(p.id, [...((v as unknown[]) ?? []), ...up.map((a) => ({ assetId: a.id, name: a.fileName }))]);
                      }} />
                    </div>
                  );
                  break;
                default:
                  input = p.type === "text"
                    ? <textarea id={idAttr} className="textarea" rows={3} value={String(v ?? "")} onChange={(e) => set(p.id, e.target.value)} />
                    : <input id={idAttr} className="input" type={p.type === "email" ? "email" : p.type === "url" ? "url" : "text"} value={String(v ?? "")} onChange={(e) => set(p.id, e.target.value)} />;
              }
              return (
                <div key={p.id} className="field" style={{ marginBottom: 16 }}>
                  {p.type !== "checkbox" && <label htmlFor={idAttr}>{f.label}{f.required && <span style={{ color: "var(--danger)" }}> *</span>}</label>}
                  {input}
                  {f.help && <span className="small-text faint">{f.help}</span>}
                  {err && <span className="small-text" role="alert" style={{ color: "var(--danger)" }}>⚠ {err}</span>}
                </div>
              );
            })}
            <button className="btn primary" type="submit">{form.config.submitLabel || t("Submit")}</button>
          </form>
        )}
      </div>
    </div>
  );
}
