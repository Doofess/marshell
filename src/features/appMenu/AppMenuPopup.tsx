import { useEffect, useLayoutEffect, useRef, useState } from "react";
import { menuKey, type MenuItem } from "../../components/ContextMenu/menuKeys";
import { AuxGlyph, StatusGlyph } from "../../components/StatusGlyph/StatusGlyph";
import { usePopupFocus } from "../../lib/usePopupFocus";
import type { Os } from "../../lib/keymap";
import "../../components/ContextMenu/ContextMenu.css";
import "./AppMenu.css";
import { appMenuItems, appMenuKey, doctorItemLabel, leavesAppMenu, menuKeyCloses, menuPlace, SETTINGS_ITEMS, submenuPlace, type AppMenuRun } from "./appMenu";

type Props = {
  os: Os;
  /** The menu button's top left, in viewport pixels; none opens at the window's bottom left. */
  anchor: { left: number; top: number } | null;
  /** Unknown before the first check: no number shows. */
  doctorIssues?: number;
  onRun: (r: AppMenuRun) => void;
  onClose: (o: { refocus: boolean }) => void;
  /** The button that toggles the menu: a press on it is left to its click, which closes the menu, so the press doesn't close it first. */
  isTrigger?: (n: object) => boolean;
};

const NO_TRIGGER = () => false;

const SUB_ITEMS: MenuItem[] = SETTINGS_ITEMS.map((s) => ({ id: s.section, label: s.label }));
const view = () => ({ w: window.innerWidth, h: window.innerHeight });

/** The app menu: Settings (a submenu), History, Doctor and Departures. Real focus roves through the items. */
export function AppMenu({ os, anchor, doctorIssues, onRun, onClose, isTrigger = NO_TRIGGER }: Props) {
  const items = appMenuItems(os);
  const wrap = useRef<HTMLDivElement>(null);
  const top = useRef<HTMLUListElement>(null);
  const sub = useRef<HTMLUListElement>(null);
  const [active, setActive] = useState(0);
  const [subOpen, setSubOpen] = useState(false);
  const [subActive, setSubActive] = useState(0);
  const [pos, setPos] = useState({ x: 8, y: 8 });
  const [subPos, setSubPos] = useState({ x: 0, y: 0 });

  const topItems = () => top.current?.querySelectorAll<HTMLElement>('[role="menuitem"]');
  const subItems = () => sub.current?.querySelectorAll<HTMLElement>('[role="menuitem"]');
  const activeItem = () => (subOpen ? subItems()?.[subActive] : topItems()?.[active]);

  useLayoutEffect(() => {
    const r = top.current?.getBoundingClientRect();
    if (r) setPos(menuPlace(anchor, { w: r.width, h: r.height }, view()));
  }, [anchor]);
  useLayoutEffect(() => {
    const m = top.current?.getBoundingClientRect();
    const s = sub.current?.getBoundingClientRect();
    const it = topItems()?.[0]?.getBoundingClientRect();
    if (subOpen && m && s && it) setSubPos(submenuPlace(m, it.top, { w: s.width, h: s.height }, view()));
  }, [subOpen, pos]);

  useEffect(() => {
    activeItem()?.focus();
  }, [active, subActive, subOpen]);
  usePopupFocus(wrap, activeItem, { tab: false });

  useEffect(() => {
    const away = (e: PointerEvent) => {
      if (wrap.current && e.target && leavesAppMenu(e.target, wrap.current, isTrigger)) onClose({ refocus: false });
    };
    window.addEventListener("pointerdown", away, true);
    return () => window.removeEventListener("pointerdown", away, true);
  }, [onClose, isTrigger]);

  const openSub = () => {
    setSubActive(0);
    setSubOpen(true);
  };

  // The menu key closes the menu it opened, from either list, and puts the focus back.
  const menuKeyClose = (e: React.KeyboardEvent) => {
    if (!menuKeyCloses(e.nativeEvent, os)) return false;
    e.preventDefault();
    onClose({ refocus: true });
    return true;
  };
  const onKey = (level: "top" | "sub") => (e: React.KeyboardEvent) => {
    e.stopPropagation();
    if (menuKeyClose(e)) return;
    if (e.ctrlKey || e.metaKey) e.preventDefault();
    const k = appMenuKey(level === "top" ? menuKey(e.key, items, active, e) : menuKey(e.key, SUB_ITEMS, subActive, e), level);
    if (!k) return;
    e.preventDefault();
    if (k.do === "move") (level === "top" ? setActive : setSubActive)(k.index);
    else if (k.do === "open") openSub();
    else if (k.do === "run") onRun((level === "top" ? k.id : `settings:${k.id}`) as AppMenuRun);
    else if (k.do === "up") setSubOpen(false);
    else onClose({ refocus: true });
  };
  const onTopKey = onKey("top");
  const onSubKey = onKey("sub");
  const onBlur = (e: React.FocusEvent) => {
    if (wrap.current && leavesAppMenu(e.relatedTarget, wrap.current, isTrigger)) onClose({ refocus: false });
  };

  return (
    <div ref={wrap} style={{ display: "contents" }}>
      <ul
        ref={top}
        className="context-menu app-menu"
        role="menu"
        aria-label="Menu"
        tabIndex={-1}
        style={{ position: "fixed", insetInlineStart: pos.x, insetBlockStart: pos.y }}
        onKeyDown={onTopKey}
        onBlur={onBlur}
        onContextMenu={(e) => e.preventDefault()}
      >
        {items.map((it, i) => {
          if ("separator" in it) return null;
          const isSettings = it.id === "settings";
          const isDoctor = it.id === "doctor";
          return (
            <li
              key={it.id}
              role="menuitem"
              tabIndex={-1}
              aria-keyshortcuts={it.shortcut}
              aria-haspopup={isSettings ? "menu" : undefined}
              aria-expanded={isSettings ? subOpen : undefined}
              aria-label={isDoctor && doctorIssues !== undefined ? doctorItemLabel(doctorIssues) : undefined}
              onClick={() => {
                if (!isSettings) return onRun(it.id as AppMenuRun);
                setActive(i);
                openSub();
              }}
              onPointerMove={() => active !== i && !subOpen && setActive(i)}
            >
              <span className="context-menu__label">{it.label}</span>
              {isSettings && (
                <span className="app-menu__more" aria-hidden="true">
                  ›
                </span>
              )}
              {isDoctor && doctorIssues !== undefined && (
                <span className="app-menu__count" aria-hidden="true">
                  {doctorIssues > 0 ? <AuxGlyph kind="caution" /> : <StatusGlyph kind="done-seen" size={12} />}
                  {doctorIssues}
                </span>
              )}
              {it.shortcut && (
                <span className="context-menu__shortcut" aria-hidden="true">
                  {it.shortcut}
                </span>
              )}
            </li>
          );
        })}
      </ul>
      {subOpen && (
        <ul
          ref={sub}
          className="context-menu app-menu"
          role="menu"
          aria-label="Settings"
          tabIndex={-1}
          style={{ position: "fixed", insetInlineStart: subPos.x, insetBlockStart: subPos.y }}
          onKeyDown={onSubKey}
          onBlur={onBlur}
          onContextMenu={(e) => e.preventDefault()}
        >
          {SUB_ITEMS.map((it, i) =>
            "separator" in it ? null : (
              <li
                key={it.id}
                role="menuitem"
                tabIndex={-1}
                onClick={() => onRun(`settings:${it.id}` as AppMenuRun)}
                onPointerMove={() => subActive !== i && setSubActive(i)}
              >
                <span className="context-menu__label">{it.label}</span>
              </li>
            ),
          )}
        </ul>
      )}
    </div>
  );
}
