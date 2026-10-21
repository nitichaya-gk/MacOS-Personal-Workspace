import { useEffect, useMemo, useState } from "react";
import { useStore } from "../../app/providers/store";
import { useI18n } from "../../app/providers/i18n";
import { api } from "../../services/desktop-bridge/api";
import type { SearchHit } from "../../shared/types";
import { addDays, debounce, relativeTime } from "../../shared/utils";
import { kindIcon } from "../../components/ui/Icon";

const TYPES = [
  { id: "page", label: "Pages" },
  { id: "project", label: "Projects" },
  { id: "record", label: "Records" },
  { id: "database", label: "Databases" },
  { id: "canvas", label: "Canvases" },
  { id: "task", label: "Tasks" },
];

export function SearchView({ initial }: { initial?: string }) {
  const s = useStore();
  const { t, locale } = useI18n();
  const [q, setQ] = useState(initial ?? "");
  const [types, setTypes] = useState<string[]>([]);
  const [projectId, setProjectId] = useState("");
  const [since, setSince] = useState("");
  const [hits, setHits] = useState<SearchHit[] | null>(null);
  const [ms, setMs] = useState(0);
  const [sel, setSel] = useState(0);
  const projects = s.tree.filter((p) => p.kind === "project" && !p.isTemplate);

  const run = useMemo(
    () => debounce((query: string, f: { types: string[]; projectId: string; since: string }) => {
      if (!query.trim()) return setHits(null);
      const started = performance.now();
      const today = s.counts?.date ?? new Date().toISOString().slice(0, 10);
      const modifiedAfter = f.since ? addDays(today, -Number(f.since)) : null;
      api.search(query, { types: f.types, projectId: f.projectId || null, modifiedAfter, limit: 100 })
        .then((h) => (setHits(h), setMs(Math.round(performance.now() - started)), setSel(0)))
        .catch(s.fail);
    }, 150),
    [s.counts?.date, s.fail],
  );
  useEffect(() => run(q, { types, projectId, since }), [q, types, projectId, since, run]);

  const open = (h: SearchHit) => (h.entityType === "task" ? s.openTask(h.entityId) : s.navigate({ name: "page", id: h.entityId }));

  return (
    <div className="content">
      <div className="page">
        <h1 style={{ margin: "16px 0 12px", fontSize: 30 }}>🔎 {t("Search")}</h1>
        <input
          className="input"
          style={{ height: 40, fontSize: 16 }}
          autoFocus
          placeholder={t("Search titles and content (Thai and English)")}
          value={q}
          onChange={(e) => setQ(e.target.value)}
          onKeyDown={(e) => {
            if (!hits?.length || e.nativeEvent.isComposing) return;
            if (e.key === "ArrowDown") (e.preventDefault(), setSel((i) => Math.min(hits.length - 1, i + 1)));
            if (e.key === "ArrowUp") (e.preventDefault(), setSel((i) => Math.max(0, i - 1)));
            if (e.key === "Enter") open(hits[sel]);
          }}
          aria-label={t("Search")}
        />
        <div className="row wrap" style={{ margin: "10px 0 14px" }}>
          {TYPES.map((x) => (
            <button key={x.id} className={`btn small${types.includes(x.id) ? " primary" : ""}`} aria-pressed={types.includes(x.id)}
              onClick={() => setTypes((ts) => (ts.includes(x.id) ? ts.filter((y) => y !== x.id) : [...ts, x.id]))}>
              {t(x.label)}
            </button>
          ))}
          <select className="select" style={{ width: 180, height: 24, fontSize: 12 }} value={projectId} onChange={(e) => setProjectId(e.target.value)} aria-label={t("Project")}>
            <option value="">{t("All projects")}</option>
            {projects.map((p) => <option key={p.id} value={p.id}>{p.title || t("Untitled")}</option>)}
          </select>
          <select className="select" style={{ width: 160, height: 24, fontSize: 12 }} value={since} onChange={(e) => setSince(e.target.value)} aria-label={t("Edited")}>
            <option value="">{t("Any time")}</option>
            <option value="1">{t("Edited in the last day")}</option>
            <option value="7">{t("Edited in the last 7 days")}</option>
            <option value="30">{t("Edited in the last 30 days")}</option>
          </select>
        </div>
        {hits === null ? (
          <p className="muted">{t("Type at least one character. Search works offline and finds Thai words inside sentences.")}</p>
        ) : hits.length === 0 ? (
          <p className="muted">{t("No results for “{q}”.", { q })}</p>
        ) : (
          <>
            <div className="small-text faint" style={{ marginBottom: 6 }}>{t("{n} results in {ms} ms", { n: hits.length, ms })}</div>
            {hits.map((h, i) => (
              <div key={h.entityId} className={`search-hit${i === sel ? " selected" : ""}`} onClick={() => open(h)} onMouseEnter={() => setSel(i)} role="link">
                <span style={{ fontSize: 18 }}>{h.icon || kindIcon(h.entityType)}</span>
                <div style={{ minWidth: 0 }}>
                  <div style={{ fontWeight: 500 }}>{h.title || t("Untitled")}</div>
                  {h.snippet && <div className="snip">{h.snippet}</div>}
                  <div className="small-text faint">{t(TYPES.find((x) => x.id === h.entityType)?.label ?? h.entityType)} · {relativeTime(h.updatedAt, locale)}</div>
                </div>
              </div>
            ))}
          </>
        )}
      </div>
    </div>
  );
}
