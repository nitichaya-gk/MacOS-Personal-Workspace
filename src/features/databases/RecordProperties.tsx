import { useCallback, useEffect, useState } from "react";
import { useStore } from "../../app/providers/store";
import { useI18n } from "../../app/providers/i18n";
import { api } from "../../services/desktop-bridge/api";
import type { DatabaseInfo, DbRecord } from "../../shared/types";
import { AddPropertyButton, PropertyCell, typeIcon, usePropertyMenu } from "./cells";

/** Property grid shown at the top of a database record's page. */
export function RecordProperties({ recordId }: { recordId: string }) {
  const s = useStore();
  const { t } = useI18n();
  const [ctx, setCtx] = useState<{ database: DatabaseInfo; record: DbRecord } | null>(null);
  const load = useCallback(() => api.databases.recordContext(recordId).then(setCtx).catch(s.fail), [recordId, s.fail]);
  useEffect(() => {
    load();
  }, [load]);
  const menu = usePropertyMenu(ctx?.database ?? null, load);
  if (!ctx) return null;
  const { database: db, record } = ctx;
  return (
    <div>
      <div className="small-text muted" style={{ marginBottom: 6 }}>
        <button className="crumb muted" onClick={() => s.navigate({ name: "page", id: db.id })}>{db.icon || "🗂️"} {db.title || t("Untitled")}</button>
      </div>
      <div className="props-grid">
        {db.properties.filter((p) => p.type !== "title").map((p) => (
          <div key={p.id} style={{ display: "contents" }}>
            <button className="pname crumb" style={{ maxWidth: "none", textAlign: "left" }} onClick={(e) => menu.open(e.currentTarget, p)}>
              <span style={{ width: 16, textAlign: "center" }}>{typeIcon(p.type)}</span> {p.name}
            </button>
            <div className="pval">
              <PropertyCell db={db} p={p} rec={record} onSaved={(r) => setCtx({ database: db, record: r })}>
                {record.values[p.id] === undefined && !["created_time", "last_edited_time", "checkbox"].includes(p.type) && <span className="faint">{t("Empty")}</span>}
              </PropertyCell>
            </div>
          </div>
        ))}
        <div style={{ gridColumn: "1 / 3" }}>
          <AddPropertyButton db={db} onAdded={load} label={t("Add property")} />
        </div>
      </div>
      {menu.element}
    </div>
  );
}
