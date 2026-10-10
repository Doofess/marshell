import { useEffect, type RefObject } from "react";
import { trapKey } from "./focusTrap";

const FOCUSABLE = 'a[href], button:not([disabled]), input:not([disabled]), select:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex="-1"])';

/**
 * The focus behaviour of a modal dialog, in the app (a static page has no effects): on open, focus moves to the title (not to
 * an action, so Enter cannot apply anything by reflex); Tab stays inside; Escape closes; on close, focus goes back to whatever
 * opened it. The key decisions are the pure `trapKey`.
 */
export function useDialogFocus(dialog: RefObject<HTMLElement | null>, title: RefObject<HTMLElement | null>, onClose?: () => void) {
  useEffect(() => {
    const el = dialog.current;
    if (!el) return;
    const opener = document.activeElement instanceof HTMLElement ? document.activeElement : null;
    title.current?.focus();
    const onKey = (e: KeyboardEvent) => {
      const controls = [...el.querySelectorAll<HTMLElement>(FOCUSABLE)];
      const action = trapKey(e, controls.length, controls.indexOf(document.activeElement as HTMLElement));
      if (!action) return;
      e.preventDefault();
      if (action.kind === "focus") controls[action.index]?.focus();
      else if (action.kind === "close") onClose?.();
    };
    el.addEventListener("keydown", onKey);
    return () => {
      el.removeEventListener("keydown", onKey);
      opener?.focus();
    };
  }, [dialog, title, onClose]);
}
