import { useEffectEvent, useLayoutEffect, type RefObject } from "react";
import { guardDom, holdFocus, tabbableIn } from "./focusGuard";
import { trapKey } from "./focusTrap";

/**
 * Keeps the focus inside an open popup (a menu, a sheet, a popover), in the app: focus that lands outside it comes back to the
 * last control it held, or to `fallback()` (see `holdFocus`). With `tab` (the default), Tab and Shift+Tab also wrap among its
 * tabbable controls, so Tab can't walk out of it. `enabled: false` (a popover opened by the pointer, a static page) does nothing.
 * `onPressOutside` is for a popup with no backdrop: a press outside it calls it (to close) rather than pulling the focus back.
 * The guard is a layout effect, so it ends before a closing popup's successor (an opener's refocus, Rename's box) takes the focus.
 */
export function usePopupFocus(
  popup: RefObject<HTMLElement | null>,
  fallback: () => HTMLElement | null | undefined,
  { tab = true, enabled = true, onPressOutside }: { tab?: boolean; enabled?: boolean; onPressOutside?: () => void } = {},
) {
  const back = useEffectEvent(() => fallback() ?? null);
  const pressed = useEffectEvent(() => onPressOutside?.());
  const closesOnPress = onPressOutside !== undefined;
  useLayoutEffect(() => {
    const el = popup.current;
    if (!el || !enabled) return;
    const stop = holdFocus(el, guardDom(el, document, window), back, undefined, closesOnPress ? pressed : undefined);
    if (!tab) return stop;
    const onKey = (e: KeyboardEvent) => {
      if (e.key !== "Tab" || e.ctrlKey || e.altKey || e.metaKey) return;
      const controls = tabbableIn(el);
      const action = trapKey(e, controls.length, controls.indexOf(document.activeElement as HTMLElement));
      if (action?.kind !== "focus" && action?.kind !== "stay") return;
      e.preventDefault();
      if (action.kind === "focus") controls[action.index]?.focus();
    };
    el.addEventListener("keydown", onKey);
    return () => {
      el.removeEventListener("keydown", onKey);
      stop();
    };
  }, [popup, enabled, tab, closesOnPress]);
}
