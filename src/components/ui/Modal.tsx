import { useEffect, useRef, useState, type ReactNode } from "react";
import { createPortal } from "react-dom";
import { useStore } from "../../app/providers/store";
import { useI18n } from "../../app/providers/i18n";

export function Modal({ title, onClose, children, footer, wide, className }: { title?: ReactNode; onClose: () => void; children: ReactNode; footer?: ReactNode; wide?: boolean; className?: string }) {
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        e.stopPropagation();
        onClose();
      }
    };
    document.addEventListener("keydown", onKey, true);
    return () => document.removeEventListener("keydown", onKey, true);
  }, [onClose]);
  return createPortal(
    <div className="overlay" onMouseDown={(e) => e.target === e.currentTarget && onClose()}>
      <div className={`modal${wide ? " wide" : ""} ${className ?? ""}`} role="dialog" aria-modal="true" aria-label={typeof title === "string" ? title : undefined}>
        {title && <div className="modal-head">{title}</div>}
        <div className="modal-body">{children}</div>
        {footer && <div className="modal-foot">{footer}</div>}
      </div>
    </div>,
    document.body,
  );
}

/** Renders the confirm/prompt request held in the store. */
export function DialogHost() {
  const { dialog, closeDialog } = useStore();
  const { t } = useI18n();
  const [value, setValue] = useState("");
  const inputRef = useRef<HTMLInputElement>(null);
  const okRef = useRef<HTMLButtonElement>(null);
  useEffect(() => {
    if (!dialog) return;
    setValue(dialog.defaultValue ?? "");
    setTimeout(() => (dialog.kind === "prompt" ? inputRef.current?.select() : okRef.current?.focus()), 0);
  }, [dialog]);
  if (!dialog) return null;
  const submit = () => closeDialog(dialog.kind === "prompt" ? value : true);
  return (
    <Modal
      title={dialog.title}
      onClose={() => closeDialog(dialog.kind === "prompt" ? null : false)}
      footer={
        <>
          <button className="btn" onClick={() => closeDialog(dialog.kind === "prompt" ? null : false)}>{t("Cancel")}</button>
          <button ref={okRef} className={`btn ${dialog.danger ? "danger solid" : "primary"}`} onClick={submit}>
            {dialog.confirmLabel ?? t("OK")}
          </button>
        </>
      }
    >
      {dialog.message && <p className="muted" style={{ marginTop: 0, whiteSpace: "pre-wrap" }}>{dialog.message}</p>}
      {dialog.kind === "prompt" && (
        <input
          ref={inputRef}
          className="input"
          value={value}
          placeholder={dialog.placeholder}
          onChange={(e) => setValue(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Enter" && !e.nativeEvent.isComposing) submit();
          }}
        />
      )}
    </Modal>
  );
}

export function Toasts() {
  const { toasts, dismissToast } = useStore();
  return createPortal(
    <div className="toasts" role="status" aria-live="polite">
      {toasts.map((t) => (
        <div key={t.id} className={`toast ${t.kind}`}>
          <span>{t.message}</span>
          {t.action && (
            <button
              onClick={() => {
                t.action!.run();
                dismissToast(t.id);
              }}
            >
              {t.action.label}
            </button>
          )}
        </div>
      ))}
    </div>,
    document.body,
  );
}

export function EmptyState({ icon, title, hint, children }: { icon?: string; title: string; hint?: string; children?: ReactNode }) {
  return (
    <div className="empty">
      {icon && <div className="big" aria-hidden>{icon}</div>}
      <div style={{ fontWeight: 500, color: "var(--text)" }}>{title}</div>
      {hint && <div className="small-text" style={{ marginTop: 4 }}>{hint}</div>}
      {children && <div className="actions">{children}</div>}
    </div>
  );
}

const EMOJIS = "📄 📝 📚 📁 🗂️ 🚀 🎯 ✅ 🗓️ 💡 📌 ⭐️ 🔥 🧠 🎨 🧭 📐 🛠️ 📊 📈 💼 🏠 🌱 🌟 ❤️ 🔒 🔑 📦 🧩 🧪 📷 🎬 🎵 ✈️ 🍜 ☕️ 🏃 💰 📮 🔔 ⚙️ 🧾 📋 🗒️ 🗃️ 🖥️ 📱 🌏 🇹🇭".split(" ");

export function EmojiPicker({ onPick, onRemove }: { onPick: (e: string) => void; onRemove?: () => void }) {
  const { t } = useI18n();
  const [custom, setCustom] = useState("");
  return (
    <div style={{ padding: 6, width: 280 }}>
      <div style={{ display: "grid", gridTemplateColumns: "repeat(8, 1fr)", gap: 2 }}>
        {EMOJIS.map((e) => (
          <button key={e} className="icon-btn" style={{ fontSize: 18 }} onClick={() => onPick(e)} aria-label={e}>
            {e}
          </button>
        ))}
      </div>
      <div className="row" style={{ marginTop: 8 }}>
        <input className="input" placeholder={t("Type or paste an emoji")} value={custom} onChange={(e) => setCustom(e.target.value)}
          onKeyDown={(e) => e.key === "Enter" && custom.trim() && onPick(custom.trim())} />
        {onRemove && <button className="btn small" onClick={onRemove}>{t("Remove")}</button>}
      </div>
    </div>
  );
}
