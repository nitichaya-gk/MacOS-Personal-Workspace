import { useEffect, useMemo, useState } from "react";
import { open as openDialog, save } from "@tauri-apps/plugin-dialog";
import { isPermissionGranted, requestPermission, sendNotification } from "@tauri-apps/plugin-notification";
import { useStore } from "../../app/providers/store";
import { useI18n } from "../../app/providers/i18n";
import { api } from "../../services/desktop-bridge/api";
import type { BackupInfo, Settings } from "../../shared/types";
import { formatBytes, formatDateTime } from "../../shared/utils";
import { kindIcon } from "../../components/ui/Icon";

type Tab = "general" | "workspace" | "backup" | "mcp";

export function SettingsView({ initialTab }: { initialTab?: string }) {
  const { t } = useI18n();
  const [tab, setTab] = useState<Tab>((initialTab as Tab) ?? "general");
  const tabs: { id: Tab; label: string; icon: string }[] = [
    { id: "general", label: t("General"), icon: "⚙️" },
    { id: "workspace", label: t("Workspace"), icon: "🗄️" },
    { id: "backup", label: t("Backup & export"), icon: "💾" },
    { id: "mcp", label: t("MCP (Claude, Codex)"), icon: "🔌" },
  ];
  return (
    <div className="content">
      <div className="page" style={{ maxWidth: 1000 }}>
        <h1 style={{ margin: "16px 0 18px", fontSize: 30 }}>{t("Settings")}</h1>
        <div className="settings">
          <nav className="settings-nav" aria-label={t("Settings")}>
            {tabs.map((x) => (
              <button key={x.id} className={`nav-item${tab === x.id ? " active" : ""}`} onClick={() => setTab(x.id)} aria-current={tab === x.id}>
                <span className="nav-icon">{x.icon}</span><span className="label">{x.label}</span>
              </button>
            ))}
          </nav>
          <div className="settings-body">
            {tab === "general" && <General />}
            {tab === "workspace" && <WorkspaceTab />}
            {tab === "backup" && <BackupTab />}
            {tab === "mcp" && <McpTab />}
          </div>
        </div>
      </div>
    </div>
  );
}

function useSetting() {
  const s = useStore();
  return <K extends keyof Settings>(key: K, value: Settings[K]) => api.settings.set(key, value).then(s.setSettings).catch(s.fail);
}

function General() {
  const s = useStore();
  const { t } = useI18n();
  const set = useSetting();
  const st = s.settings!;
  const zones = useMemo(() => {
    try {
      return (Intl as unknown as { supportedValuesOf: (k: string) => string[] }).supportedValuesOf("timeZone");
    } catch {
      return ["UTC", "Asia/Bangkok"];
    }
  }, []);
  const [perm, setPerm] = useState<boolean | null>(null);
  useEffect(() => {
    isPermissionGranted().then(setPerm).catch(() => setPerm(null));
  }, []);

  return (
    <>
      <section className="card-box">
        <h3>{t("Language")}</h3>
        <p className="desc">{t("Interface language. Thai input works everywhere regardless of this setting.")}</p>
        <select className="select" style={{ width: 240 }} value={st.language} onChange={(e) => set("language", e.target.value as Settings["language"])}>
          <option value="system">{t("Same as macOS")}</option>
          <option value="en">English</option>
          <option value="th">ไทย</option>
        </select>
      </section>
      <section className="card-box">
        <h3>{t("Time zone")}</h3>
        <p className="desc">{t("Decides which tasks are Today and Overdue. Reminders are stored as exact moments, so they fire at the same instant even if you travel or change this.")}</p>
        <select className="select" style={{ width: 320 }} value={st.timezone} onChange={(e) => set("timezone", e.target.value).then(() => s.refreshCounts())}>
          <option value="system">{t("Same as macOS ({tz})", { tz: st.timezone === "system" ? st.effectiveTimezone : Intl.DateTimeFormat().resolvedOptions().timeZone })}</option>
          {zones.map((z) => <option key={z} value={z}>{z}</option>)}
        </select>
      </section>
      <section className="card-box">
        <h3>{t("Notifications")}</h3>
        <p className="desc">
          {t("Reminders and the daily summary use macOS notifications while the app is running. Reminders that came due while the app was closed are summarised once when it opens.")}
        </p>
        <div className="row wrap">
          <span>{perm === true ? `✅ ${t("Allowed")}` : perm === false ? `⛔️ ${t("Not allowed")}` : "—"}</span>
          <button className="btn small" onClick={async () => setPerm((await requestPermission()) === "granted")}>{t("Ask for permission")}</button>
          <button className="btn small" onClick={() => sendNotification({ title: "Personal Workspace", body: t("Notifications are working.") })}>{t("Send test notification")}</button>
        </div>
        <label className="row" style={{ marginTop: 14 }}>
          <input type="checkbox" checked={st.dailySummary.enabled} onChange={(e) => set("dailySummary", { ...st.dailySummary, enabled: e.target.checked })} />
          {t("Daily summary of today's tasks at")}
          <input className="input" type="time" style={{ width: 120 }} value={st.dailySummary.time} onChange={(e) => e.target.value && set("dailySummary", { ...st.dailySummary, time: e.target.value })} />
        </label>
      </section>
      <section className="card-box">
        <h3>{t("Keyboard shortcuts")}</h3>
        <div className="kv">
          {[["⌘K", t("Search & commands")], ["⌘P", t("Advanced search")], ["⌘N", t("New page")], ["⌘⇧T", t("New task")], ["⌘\\", t("Toggle sidebar")], ["⌘⇧I", t("Toggle inspector")], ["⌘[ / ⌘]", t("Back / Forward")], ["⌘,", t("Settings")], ["/", t("Block menu in the editor")], ["@", t("Link a page in the editor")], ["⌘Z / ⌘⇧Z", t("Undo / Redo")]].map(([k, v]) => (
            <div key={k} style={{ display: "contents" }}><span className="k"><span className="kbd">{k}</span></span><span>{v}</span></div>
          ))}
        </div>
      </section>
    </>
  );
}

function WorkspaceTab() {
  const s = useStore();
  const { t } = useI18n();
  const info = s.info!;
  const [integrity, setIntegrity] = useState<Record<string, unknown> | null>(null);
  const switchTo = async (root: string, create: boolean) => {
    try {
      await (create ? api.workspace.create(root) : api.workspace.switchTo(root));
      await s.refreshAll();
      s.navigate({ name: "home" });
      s.toast(t("Workspace opened"), { kind: "success" });
    } catch (e) {
      s.fail(e);
    }
  };
  return (
    <>
      <section className="card-box">
        <h3>{t("This workspace")}</h3>
        <div className="row" style={{ margin: "8px 0 12px" }}>
          <input className="input" style={{ width: 280 }} defaultValue={info.name} aria-label={t("Workspace name")}
            onBlur={(e) => e.target.value.trim() && e.target.value !== info.name && api.workspace.rename(e.target.value).then(() => s.refreshAll()).catch(s.fail)} />
        </div>
        <div className="kv">
          <span className="k">{t("Location")}</span>
          <span className="row" style={{ minWidth: 0 }}><span className="mono selectable" style={{ wordBreak: "break-all" }}>{info.root}</span><button className="btn small" onClick={() => api.system.reveal(`${info.root}/workspace.sqlite`)}>{t("Show in Finder")}</button></span>
          <span className="k">{t("Pages")}</span><span>{info.pages}</span>
          <span className="k">{t("Tasks")}</span><span>{info.tasks}</span>
          <span className="k">{t("Attachments")}</span><span>{formatBytes(info.assetsBytes)}</span>
          <span className="k">{t("Schema version")}</span><span>{info.schemaVersion}</span>
          <span className="k">{t("Storage")}</span><span className="small-text muted">{t("SQLite database + assets/ folder + manifest.json. Nothing leaves this Mac unless you export it.")}</span>
        </div>
      </section>
      <section className="card-box">
        <h3>{t("Open or create a workspace")}</h3>
        <p className="desc">{t("Keep separate workspaces in any folder you choose, e.g. on an external disk.")}</p>
        <div className="row wrap">
          <button className="btn" onClick={async () => { const d = await openDialog({ directory: true }); if (typeof d === "string") switchTo(d, false); }}>{t("Open workspace folder…")}</button>
          <button className="btn" onClick={async () => { const d = await openDialog({ directory: true, title: t("Choose an empty folder for the new workspace") }); if (typeof d === "string") switchTo(d, true); }}>{t("New workspace…")}</button>
        </div>
        {info.recentWorkspaces.filter((r) => r !== info.root).length > 0 && (
          <>
            <div className="menu-label" style={{ paddingLeft: 0, marginTop: 12 }}>{t("Recent")}</div>
            {info.recentWorkspaces.filter((r) => r !== info.root).map((r) => (
              <button key={r} className="menu-item mono" onClick={() => switchTo(r, false)}>{r}</button>
            ))}
          </>
        )}
      </section>
      <section className="card-box">
        <h3>{t("Maintenance")}</h3>
        <p className="desc">{t("Check the database, attachments and internal links, or rebuild the search index (derived data).")}</p>
        <div className="row wrap">
          <button className="btn" onClick={() => api.workspace.integrity().then(setIntegrity).catch(s.fail)}>{t("Check integrity")}</button>
          <button className="btn" onClick={() => api.workspace.rebuildIndex().then((r) => s.toast(t("Indexed {n} items", { n: r.indexed }), { kind: "success" })).catch(s.fail)}>{t("Rebuild search index")}</button>
        </div>
        {integrity && (
          <div className="kv" style={{ marginTop: 12 }}>
            <span className="k">{t("Result")}</span><span>{integrity.ok ? `✅ ${t("No problems found")}` : `⚠️ ${t("Problems found")}`}</span>
            <span className="k">SQLite</span><span>{String(integrity.sqlite)}</span>
            <span className="k">{t("Missing attachment files")}</span><span>{(integrity.missingAssetFiles as unknown[]).length + (integrity.missingReferencedAssets as unknown[]).length}</span>
            <span className="k">{t("Unused attachment files")}</span><span>{String(integrity.orphanAssetFiles)}</span>
            <span className="k">{t("Links to deleted items")}</span><span>{String(integrity.brokenLinks)}</span>
          </div>
        )}
      </section>
    </>
  );
}

function BackupTab() {
  const s = useStore();
  const { t, locale } = useI18n();
  const set = useSetting();
  const st = s.settings!;
  const [list, setList] = useState<BackupInfo[]>([]);
  const [busy, setBusy] = useState(false);
  const load = () => api.backup.list().then(setList).catch(s.fail);
  useEffect(() => {
    load();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const restore = async (source: string) => {
    const parent = await openDialog({ directory: true, title: t("Choose where to put the restored copy") });
    if (typeof parent !== "string") return;
    const stamp = new Date().toISOString().slice(0, 16).replace(/[:T]/g, "-");
    setBusy(true);
    try {
      const r = await api.backup.restore(source, `${parent}/Restored workspace ${stamp}`);
      const ok = r.integrity.ok;
      if (await s.confirm({
        title: ok ? t("Backup restored") : t("Backup restored with warnings"),
        message: t("The copy is at {path}. Your current workspace was not changed. Open the restored copy now?", { path: r.path }),
        confirmLabel: t("Open restored workspace"),
      })) {
        await api.workspace.switchTo(r.path);
        await s.refreshAll();
        s.navigate({ name: "home" });
      }
    } catch (e) {
      s.fail(e);
    } finally {
      setBusy(false);
    }
  };

  return (
    <>
      <section className="card-box">
        <h3>{t("Backups")}</h3>
        <p className="desc">{t("A consistent snapshot of the database plus all attachments, stored inside the workspace folder. These protect against mistakes, not against disk failure — also export a copy to another drive.")}</p>
        <div className="row wrap" style={{ marginBottom: 12 }}>
          <button className="btn primary" disabled={busy} onClick={async () => { setBusy(true); await api.backup.create().then(() => (load(), s.toast(t("Backup created"), { kind: "success" }))).catch(s.fail); setBusy(false); }}>{t("Back up now")}</button>
          <label className="row"><input type="checkbox" checked={st.backup.auto} onChange={(e) => set("backup", { ...st.backup, auto: e.target.checked })} /> {t("Automatically every day")}</label>
          <label className="row">{t("Keep")} <input className="input" type="number" min={1} max={100} style={{ width: 70 }} value={st.backup.keep} onChange={(e) => Number(e.target.value) >= 1 && set("backup", { ...st.backup, keep: Number(e.target.value) })} /> {t("backups")}</label>
        </div>
        {list.length ? (
          <table className="plain">
            <thead><tr><th>{t("Backup")}</th><th>{t("Size")}</th><th /></tr></thead>
            <tbody>
              {list.map((b) => (
                <tr key={b.path}>
                  <td>{b.createdAt ? formatDateTime(b.createdAt, locale) : b.name}{b.name.endsWith("-auto") && <span className="chip" style={{ marginLeft: 6 }}>{t("auto")}</span>}</td>
                  <td className="muted">{formatBytes(b.sizeBytes)}</td>
                  <td style={{ textAlign: "right" }}>
                    <button className="btn small" onClick={() => api.system.reveal(b.path)}>{t("Show")}</button>{" "}
                    <button className="btn small" disabled={busy} onClick={() => restore(b.path)}>{t("Restore…")}</button>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        ) : (
          <div className="muted">{t("No backups yet.")}</div>
        )}
      </section>
      <section className="card-box">
        <h3>{t("Export & restore elsewhere")}</h3>
        <p className="desc">{t("Export the whole workspace (structure, ids, links and attachments) as one .zip for another disk or Mac. Restoring always goes to a new folder so you can check it first.")}</p>
        <div className="row wrap">
          <button className="btn" disabled={busy} onClick={async () => {
            const dest = await save({ defaultPath: `${s.info?.name ?? "Workspace"} ${new Date().toISOString().slice(0, 10)}.zip`, filters: [{ name: "Zip", extensions: ["zip"] }] });
            if (!dest) return;
            setBusy(true);
            await api.backup.exportZip(dest).then((r) => s.toast(t("Exported {size}", { size: formatBytes(r.sizeBytes) }), { kind: "success" })).catch(s.fail);
            setBusy(false);
          }}>{t("Export workspace (.zip)…")}</button>
          <button className="btn" disabled={busy} onClick={async () => {
            const f = await openDialog({ multiple: false, filters: [{ name: "Workspace backup", extensions: ["zip"] }] });
            if (typeof f === "string") restore(f);
          }}>{t("Restore from .zip…")}</button>
        </div>
        <p className="small-text faint" style={{ marginTop: 10 }}>{t("Pages export to Markdown/PDF from their ⋯ menu, databases to CSV/JSON from the database menu, and canvases to PNG/SVG/JSON from the canvas sidebar.")}</p>
      </section>
    </>
  );
}

function McpTab() {
  const s = useStore();
  const { t, locale } = useI18n();
  const set = useSetting();
  const mcp = s.settings!.mcp;
  const [status, setStatus] = useState<Awaited<ReturnType<typeof api.mcp.status>> | null>(null);
  useEffect(() => {
    api.mcp.status().then(setStatus).catch(s.fail);
  }, [s.fail, mcp]);
  const toggle = (id: string) => set("mcp", { ...mcp, allowedIds: mcp.allowedIds.includes(id) ? mcp.allowedIds.filter((x) => x !== id) : [...mcp.allowedIds, id] });
  const shareable = s.tree.filter((p) => !p.parentId && (p.kind === "project" || p.kind === "database" || p.kind === "page"));
  const copy = (text: string) => navigator.clipboard.writeText(text).then(() => s.toast(t("Copied")));

  return (
    <>
      <section className="card-box">
        <h3>{t("Let AI clients read this workspace")}</h3>
        <p className="desc">{t("Claude Desktop, Claude Code, Codex and other MCP clients can search and read what you share here. Access is read-only. The workspace stays on this Mac, but content a client reads may be sent to that client's AI service according to its own settings.")}</p>
        <label className="row" style={{ marginBottom: 10 }}>
          <input type="checkbox" checked={mcp.enabled} onChange={(e) => set("mcp", { ...mcp, enabled: e.target.checked })} />
          <strong>{t("Enable MCP access")}</strong>
        </label>
        <div className="row" style={{ marginBottom: 10, opacity: mcp.enabled ? 1 : 0.5 }}>
          <label className="row"><input type="radio" name="scope" checked={mcp.scope === "selected"} onChange={() => set("mcp", { ...mcp, scope: "selected" })} disabled={!mcp.enabled} /> {t("Only what I select")}</label>
          <label className="row"><input type="radio" name="scope" checked={mcp.scope === "all"} onChange={() => set("mcp", { ...mcp, scope: "all" })} disabled={!mcp.enabled} /> {t("Everything (except databases marked hidden)")}</label>
        </div>
        {mcp.enabled && mcp.scope === "selected" && (
          <div style={{ maxHeight: 280, overflowY: "auto", border: "1px solid var(--border)", borderRadius: 8, padding: 6 }}>
            <div className="menu-label">{t("Sections")}</div>
            {s.sections.filter((x) => x.kind !== "templates").map((x) => (
              <label key={x.id} className="menu-item"><input type="checkbox" checked={mcp.allowedIds.includes(x.id)} onChange={() => toggle(x.id)} /> {x.icon} {x.name}</label>
            ))}
            <label className="menu-item"><input type="checkbox" checked={mcp.allowedIds.includes("tasks:unassigned")} onChange={() => toggle("tasks:unassigned")} /> ✅ {t("Tasks without a project")}</label>
            <div className="menu-label">{t("Individual pages, projects and databases")}</div>
            {shareable.map((p) => (
              <label key={p.id} className="menu-item"><input type="checkbox" checked={mcp.allowedIds.includes(p.id)} onChange={() => toggle(p.id)} /> {p.icon || kindIcon(p.kind)} {p.title || t("Untitled")}</label>
            ))}
          </div>
        )}
      </section>
      {status && (
        <section className="card-box">
          <h3>{t("Connect a client")}</h3>
          <p className="desc">{t("The server runs over stdio from this app's own executable. Tools: search_workspace, list_projects, read_page, query_database, list_tasks, read_canvas.")}</p>
          <div className="field-label">Claude Code</div>
          <pre className="code">{status.claudeCode}</pre>
          <button className="btn small" onClick={() => copy(status.claudeCode)}>{t("Copy")}</button>
          <div className="field-label" style={{ marginTop: 12 }}>Claude Desktop — claude_desktop_config.json</div>
          <pre className="code">{JSON.stringify(status.claudeDesktop, null, 2)}</pre>
          <button className="btn small" onClick={() => copy(JSON.stringify(status.claudeDesktop, null, 2))}>{t("Copy")}</button>
          <div className="field-label" style={{ marginTop: 12 }}>Codex — ~/.codex/config.toml</div>
          <pre className="code">{status.codex}</pre>
          <button className="btn small" onClick={() => copy(status.codex)}>{t("Copy")}</button>
        </section>
      )}
      {status && (
        <section className="card-box">
          <h3>{t("Activity log")}</h3>
          <p className="desc">{t("Every MCP call is recorded (client, tool, target, result) for 90 days. Page content is not stored in the log.")}</p>
          {status.audit.length ? (
            <table className="plain">
              <thead><tr><th>{t("When")}</th><th>{t("Client")}</th><th>{t("Tool")}</th><th>{t("Target")}</th><th>{t("Result")}</th></tr></thead>
              <tbody>
                {status.audit.map((a, i) => (
                  <tr key={i}>
                    <td className="muted">{formatDateTime(a.at, locale)}</td><td>{a.client}</td><td className="mono">{a.tool}</td>
                    <td className="mono" style={{ maxWidth: 160, overflow: "hidden", textOverflow: "ellipsis" }}>{a.target}</td>
                    <td title={a.detail ?? ""}>{a.result === "ok" ? "✅ ok" : a.result === "denied" ? `⛔️ ${t("denied")}` : `⚠️ ${t("error")}`}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          ) : (
            <div className="muted">{t("No MCP activity yet.")}</div>
          )}
        </section>
      )}
    </>
  );
}
