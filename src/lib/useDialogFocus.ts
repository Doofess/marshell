import { useEffect, useEffectEvent, type RefObject } from "react";
import { dialogDom, openDialogFocus } from "./dialogFocus";
import { usePopupFocus } from "./usePopupFocus";

/**
 * The focus behaviour of a modal dialog, in the app (a static page has no effects): on open, focus moves to the title (not to
 * an action, so Enter cannot apply anything by reflex); Tab stays inside, and focus that lands outside comes back; Escape
 * closes; on close, focus goes back to whatever opened it. The lifecycle is the pure `openDialogFocus`, and it runs once per open: `onClose` is read through an effect event,
 * so a parent that re-renders (the doctor's "Checked N s ago" tick) never tears the dialog down and refocuses it.
 */
export function useDialogFocus(dialog: RefObject<HTMLElement | null>, title: RefObject<HTMLElement | null>, onClose?: () => void) {
  const close = useEffectEvent(() => onClose?.());
  useEffect(() => {
    const el = dialog.current;
    if (!el) return;
    return openDialogFocus(dialogDom(el, title.current, document), close);
  }, [dialog, title]);
  usePopupFocus(dialog, () => title.current, { tab: false });
}
