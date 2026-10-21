import { useEffect, useLayoutEffect, useRef, useState, type ReactNode } from "react";
import { createPortal } from "react-dom";

export type Anchor = HTMLElement | DOMRect | { x: number; y: number };

function rectOf(a: Anchor): DOMRect {
  if (a instanceof HTMLElement) return a.getBoundingClientRect();
  if ("width" in a) return a;
  return new DOMRect(a.x, a.y, 0, 0);
}

/** Floating panel anchored to an element or point; closes on outside click or Escape. */
export function Popover({ anchor, onClose, children, width, align = "start" }: { anchor: Anchor; onClose: () => void; children: ReactNode; width?: number; align?: "start" | "end" }) {
  const ref = useRef<HTMLDivElement>(null);
  const [pos, setPos] = useState<{ left: number; top: number } | null>(null);

  useLayoutEffect(() => {
    const r = rectOf(anchor);
    const el = ref.current;
    const w = el?.offsetWidth ?? width ?? 220;
    const h = el?.offsetHeight ?? 200;
    let left = align === "end" ? r.right - w : r.left;
    let top = r.bottom + 4;
    if (left + w > window.innerWidth - 8) left = window.innerWidth - w - 8;
    if (top + h > window.innerHeight - 8) top = Math.max(8, r.top - h - 4);
    setPos({ left: Math.max(8, left), top });
  }, [anchor, width, align]);

  useEffect(() => {
    const onDown = (e: MouseEvent) => {
      if (ref.current && !ref.current.contains(e.target as Node)) onClose();
    };
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        e.stopPropagation();
        onClose();
      }
    };
    const t = setTimeout(() => document.addEventListener("mousedown", onDown), 0);
    document.addEventListener("keydown", onKey, true);
    return () => {
      clearTimeout(t);
      document.removeEventListener("mousedown", onDown);
      document.removeEventListener("keydown", onKey, true);
    };
  }, [onClose]);

  return createPortal(
    <div ref={ref} className="popover" style={{ left: pos?.left ?? -9999, top: pos?.top ?? -9999, width }} role="dialog">
      {children}
    </div>,
    document.body,
  );
}

export interface MenuEntry {
  label?: string;
  icon?: ReactNode;
  hint?: string;
  danger?: boolean;
  disabled?: boolean;
  onSelect?: () => void;
  separator?: boolean;
  heading?: string;
}

/** Keyboard-navigable menu (↑/↓/Enter). */
export function Menu({ anchor, items, onClose, align }: { anchor: Anchor; items: MenuEntry[]; onClose: () => void; align?: "start" | "end" }) {
  const actionable = items.map((it, i) => (it.onSelect && !it.disabled ? i : -1)).filter((i) => i >= 0);
  const [sel, setSel] = useState(-1);
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "ArrowDown" || e.key === "ArrowUp") {
        e.preventDefault();
        const cur = actionable.indexOf(sel);
        const next = e.key === "ArrowDown" ? (cur + 1) % actionable.length : (cur - 1 + actionable.length) % actionable.length;
        setSel(actionable[next] ?? -1);
      } else if (e.key === "Enter" && sel >= 0) {
        e.preventDefault();
        items[sel].onSelect?.();
        onClose();
      }
    };
    document.addEventListener("keydown", onKey);
    return () => document.removeEventListener("keydown", onKey);
  });
  return (
    <Popover anchor={anchor} onClose={onClose} align={align}>
      <div role="menu">
        {items.map((it, i) =>
          it.separator ? (
            <div key={i} className="menu-sep" />
          ) : it.heading ? (
            <div key={i} className="menu-label">{it.heading}</div>
          ) : (
            <button
              key={i}
              role="menuitem"
              className={`menu-item${it.danger ? " danger" : ""}${sel === i ? " selected" : ""}`}
              disabled={it.disabled}
              style={it.disabled ? { opacity: 0.45 } : undefined}
              onMouseEnter={() => setSel(i)}
              onClick={() => {
                it.onSelect?.();
                onClose();
              }}
            >
              {it.icon}
              <span>{it.label}</span>
              {it.hint && <span className="hint">{it.hint}</span>}
            </button>
          ),
        )}
      </div>
    </Popover>
  );
}

/** Hook for "click a button → show a menu/popover" patterns. */
export function useAnchor() {
  const [anchor, setAnchor] = useState<Anchor | null>(null);
  return {
    anchor,
    open: (a: Anchor) => setAnchor(a),
    toggle: (e: { currentTarget: HTMLElement }) => setAnchor((cur) => (cur ? null : e.currentTarget)),
    close: () => setAnchor(null),
  };
}
