import { useEffect, useState } from "react";
import { useStore } from "../../app/providers/store";
import { useI18n } from "../../app/providers/i18n";
import { rpc } from "../../services/desktop-bridge/api";
import { Icon } from "../../components/ui/Icon";
import { Modal } from "../../components/ui/Modal";

interface Version {
  id: string;
  createdAt: string;
  source: string;
  title: string;
  size: number;
}
export interface Comment {
  id: string;
  blockId: string | null;
  quote: string;
  body: string;
  resolvedAt: string | null;
  createdAt: string;
}

export const historyApi = {
  versions: (pageId: string) => rpc<Version[]>("versions.list", { pageId }),
  version: (id: string) => rpc<{ id: string; title: string; markdown: string }>("versions.get", { id }),
  snapshot: (pageId: string) => rpc("versions.snapshot", { pageId }),
  restore: (id: string) => rpc<{ pageId: string }>("versions.restore", { id }),
  comments: (pageId: string) => rpc<Comment[]>("comments.list", { pageId }),
  addComment: (pageId: string, body: string, blockId?: string, quote?: string) => rpc("comments.add", { pageId, body, blockId, quote }),
  updateComment: (id: string, patch: { body?: string; resolved?: boolean }) => rpc("comments.update", { id, ...patch }),
  deleteComment: (id: string) => rpc("comments.delete", { id }),
};

/** Opened from the page menu via the `pw:history` event. */
export function VersionHistoryHost() {
  const s = useStore();
  const { t, locale } = useI18n();
  const [pageId, setPageId] = useState<string | null>(null);
  const [list, setList] = useState<Version[]>([]);
  const [sel, setSel] = useState<string | null>(null);
  const [preview, setPreview] = useState("");
  useEffect(() => {
    const on = (e: Event) => setPageId((e as CustomEvent<string>).detail);
    window.addEventListener("pw:history", on);
    return () => window.removeEventListener("pw:history", on);
  }, []);
  useEffect(() => {
    if (!pageId) return;
    historyApi.versions(pageId).then((v) => (setList(v), setSel(v[0]?.id ?? null))).catch(s.fail);
  }, [pageId, s.fail]);
  useEffect(() => {
    if (sel) historyApi.version(sel).then((v) => setPreview(v.markdown)).catch(() => setPreview(""));
    else setPreview("");
  }, [sel]);
  if (!pageId) return null;
  const source = (x: string) => (x === "autosave" ? t("Autosave") : x === "manual" ? t("Saved by you") : x === "before_restore" ? t("Before a restore") : x.startsWith("mcp:") ? `${t("Before AI edit")} (${x.slice(4)})` : x);
  return (
    <Modal title={t("Version history")} wide onClose={() => setPageId(null)}
      footer={<>
        <button className="btn" onClick={() => historyApi.snapshot(pageId).then(() => historyApi.versions(pageId).then(setList)).catch(s.fail)}>{t("Save a version now")}</button>
        <span className="spacer" />
        <button className="btn" onClick={() => setPageId(null)}>{t("Close")}</button>
        <button className="btn primary" disabled={!sel} onClick={async () => {
          if (!sel || !(await s.confirm({ title: t("Restore this version?"), message: t("The current content is saved as a version first, so you can undo this."), confirmLabel: t("Restore") }))) return;
          await historyApi.restore(sel).catch(s.fail);
          setPageId(null);
          await s.refreshTree();
          s.navigate({ name: "home" }, { replace: true });
          setTimeout(() => s.navigate({ name: "page", id: pageId }, { replace: true }), 0);
          s.toast(t("Version restored"), { kind: "success" });
        }}>{t("Restore")}</button>
      </>}>
      {list.length === 0 ? <p className="muted">{t("No versions yet. A version is kept automatically every 10 minutes of editing, before AI edits and before restores.")}</p> : (
        <div style={{ display: "grid", gridTemplateColumns: "220px 1fr", gap: 12, minHeight: 320 }}>
          <div style={{ overflowY: "auto", maxHeight: 420 }}>
            {list.map((v) => (
              <button key={v.id} className={`nav-item${sel === v.id ? " active" : ""}`} style={{ height: "auto", padding: "6px 8px", flexDirection: "column", alignItems: "flex-start" }} onClick={() => setSel(v.id)}>
                <span>{new Date(v.createdAt).toLocaleString(locale)}</span>
                <span className="small-text faint">{source(v.source)}</span>
              </button>
            ))}
          </div>
          <pre className="code" style={{ maxHeight: 420, overflowY: "auto", margin: 0 }}>{preview}</pre>
        </div>
      )}
    </Modal>
  );
}

/** Private comments list for the inspector. */
export function CommentsPanel({ pageId }: { pageId: string }) {
  const s = useStore();
  const { t, locale } = useI18n();
  const [list, setList] = useState<Comment[]>([]);
  const [text, setText] = useState("");
  const [showResolved, setShowResolved] = useState(false);
  const load = () => historyApi.comments(pageId).then(setList).catch(() => {});
  useEffect(() => {
    load();
    const on = () => load();
    window.addEventListener("pw:comments", on);
    return () => window.removeEventListener("pw:comments", on);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [pageId]);
  const open = list.filter((c) => !c.resolvedAt);
  const resolved = list.filter((c) => c.resolvedAt);
  const add = () => text.trim() && historyApi.addComment(pageId, text).then(() => (setText(""), load())).catch(s.fail);
  const item = (c: Comment) => (
    <div key={c.id} className="card-box" style={{ padding: "8px 10px", marginBottom: 6, opacity: c.resolvedAt ? 0.6 : 1 }}>
      {c.quote && <div className="small-text faint" style={{ borderLeft: "2px solid var(--border-strong)", paddingLeft: 6, marginBottom: 4 }}>{c.quote}</div>}
      <div className="selectable" style={{ whiteSpace: "pre-wrap", fontSize: 13 }}>{c.body}</div>
      <div className="row small-text faint" style={{ marginTop: 4 }}>
        <span>{new Date(c.createdAt).toLocaleString(locale)}</span>
        <span className="spacer" />
        <button className="btn small ghost" onClick={() => historyApi.updateComment(c.id, { resolved: !c.resolvedAt }).then(load).catch(s.fail)}>{c.resolvedAt ? t("Reopen") : `✓ ${t("Resolve")}`}</button>
        <button className="icon-btn small" aria-label={t("Delete comment")} onClick={() => historyApi.deleteComment(c.id).then(load).catch(s.fail)}><Icon name="trash" size={12} /></button>
      </div>
    </div>
  );
  return (
    <div>
      {open.map(item)}
      <textarea className="textarea" rows={2} placeholder={t("Add a private note…")} value={text} onChange={(e) => setText(e.target.value)}
        onKeyDown={(e) => { if (e.key === "Enter" && (e.metaKey || e.ctrlKey)) add(); }} />
      <div className="row" style={{ marginTop: 4 }}>
        {resolved.length > 0 && <button className="btn small ghost" onClick={() => setShowResolved(!showResolved)}>{showResolved ? t("Hide resolved") : t("{n} resolved", { n: resolved.length })}</button>}
        <span className="spacer" />
        <button className="btn small" onClick={add} disabled={!text.trim()}>{t("Comment")}</button>
      </div>
      {showResolved && resolved.map(item)}
    </div>
  );
}
