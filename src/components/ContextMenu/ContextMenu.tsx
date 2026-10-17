import { useEffect, useLayoutEffect, useRef, useState } from "react";
import "./ContextMenu.css";
import { leavesMenu, menuKey, placeMenu, type MenuItem } from "./menuKeys";

export type { MenuItem } from "./menuKeys";

type Props = {
  items: MenuItem[];
  /** The menu's accessible name. */
  label: string;
  /** Where it opened, in viewport pixels. */
  anchor: { x: number; y: number };
  onRun: (id: string) => void;
  onClose: (opts: { refocus: boolean }) => void;
};

const firstAction = (items: MenuItem[]) => items.findIndex((i) => !("separator" in i));

/**
 * A context menu. The active item holds real DOM focus (roving), so a screen reader reads it as it moves. It is vendor-neutral:
 * the parent must render it outside any `[data-agent]` element.
 */
export function ContextMenu({ items, label, anchor, onRun, onClose }: Props) {
  const ref = useRef<HTMLUListElement>(null);
  const [active, setActive] = useState(() => firstAction(items));
  const [pos, setPos] = useState(anchor);

  // Measure once laid out, then pull the menu back inside the window.
  useLayoutEffect(() => {
    const r = ref.current?.getBoundingClientRect();
    if (r) setPos(placeMenu(anchor, { w: r.width, h: r.height }, { w: window.innerWidth, h: window.innerHeight }));
  }, [anchor]);

  // Roving focus: the active item takes DOM focus. Items are the only children with role=menuitem, so index by item order.
  useEffect(() => {
    const n = items.slice(0, active).filter((i) => !("separator" in i)).length;
    ref.current?.querySelectorAll<HTMLElement>('[role="menuitem"]')[n]?.focus();
  }, [active, items]);

  useEffect(() => {
    const away = (e: PointerEvent) => {
      if (!ref.current?.contains(e.target as Node)) onClose({ refocus: false });
    };
    window.addEventListener("pointerdown", away, true);
    return () => window.removeEventListener("pointerdown", away, true);
  }, [onClose]);

  const onKeyDown = (e: React.KeyboardEvent) => {
    // The open menu owns every key: none reaches the window's shortcuts, which would act on the focused session, not this row.
    // A chord is also kept from the webview (Ctrl+Shift+R would reload it), since the window's own handler no longer sees it.
    e.stopPropagation();
    if (e.ctrlKey || e.metaKey) e.preventDefault();
    const r = menuKey(e.key, items, active, e);
    if (!r) return;
    // Tab keeps its default so focus moves on from where Tab puts it.
    if (!(r.kind === "close" && !r.refocus)) e.preventDefault();
    if (r.kind === "move") setActive(r.index);
    else if (r.kind === "run") onRun(r.id);
    else onClose({ refocus: r.refocus });
  };

  return (
    <ul
      ref={ref}
      className="context-menu"
      role="menu"
      aria-label={label}
      tabIndex={-1}
      style={{ position: "fixed", insetInlineStart: pos.x, insetBlockStart: pos.y }}
      onKeyDown={onKeyDown}
      // Focus wandering out (a pane or a box taking it) must not leave a stale menu up. Not refocused: focus has a new home.
      onBlur={(e) => leavesMenu(e.relatedTarget, e.currentTarget) && onClose({ refocus: false })}
      onContextMenu={(e) => e.preventDefault()}
    >
      {items.map((it, i) =>
        "separator" in it ? (
          <li key={`sep-${i}`} role="separator" className="context-menu__sep" />
        ) : (
          <li
            key={it.id}
            role="menuitem"
            tabIndex={-1}
            aria-keyshortcuts={it.shortcut}
            onClick={() => onRun(it.id)}
            onPointerMove={() => active !== i && setActive(i)}
          >
            <span className="context-menu__label">{it.label}</span>
            {it.shortcut && (
              <span className="context-menu__shortcut" aria-hidden="true">
                {it.shortcut}
              </span>
            )}
          </li>
        ),
      )}
    </ul>
  );
}
