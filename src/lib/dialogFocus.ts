import { trapKey } from "./focusTrap";

/** What a dialog's focus trap walks: links, enabled controls, and anything made focusable (a scrolling body with tabindex 0). */
export const FOCUSABLE = 'a[href], button:not([disabled]), input:not([disabled]), select:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex="-1"])';

export type Focusable = { focus(): void };

/** The little of the DOM the lifecycle touches, so it can run against a fake one in tests. */
export type DialogDom = {
  /** Where focus is now: the opener when the dialog opens, the focused control when a key arrives. */
  active(): Focusable | null;
  title: Focusable | null;
  /** The dialog's controls in document order (the FOCUSABLE query), read afresh when a key arrives. */
  controls(): Focusable[];
  /** Listens for keydown on the dialog; returns the way to stop listening. */
  onKeydown(handler: (e: { key: string; shiftKey?: boolean; preventDefault(): void }) => void): () => void;
};

/**
 * One open of a modal dialog: remember the opener, focus the title (not an action, so Enter cannot apply anything by reflex),
 * keep Tab inside and close on Escape. The returned function ends it and puts focus back on the opener. `close` is called as
 * it is at the time, so a dialog can be handed a new callback on every re-render without the lifecycle running again.
 */
export function openDialogFocus(dom: DialogDom, close: () => void): () => void {
  const opener = dom.active();
  dom.title?.focus();
  const stop = dom.onKeydown((e) => {
    const controls = dom.controls();
    const current = dom.active();
    const action = trapKey(e, controls.length, current ? controls.indexOf(current) : -1);
    if (!action) return;
    e.preventDefault();
    if (action.kind === "focus") controls[action.index]?.focus();
    else if (action.kind === "close") close();
  });
  return () => {
    stop();
    opener?.focus();
  };
}

/** The real DOM behind a dialog element. */
export function dialogDom(el: HTMLElement, title: HTMLElement | null, doc: Document): DialogDom {
  return {
    active: () => (doc.activeElement instanceof HTMLElement ? doc.activeElement : null),
    title,
    controls: () => [...el.querySelectorAll<HTMLElement>(FOCUSABLE)],
    onKeydown: (handler) => {
      el.addEventListener("keydown", handler);
      return () => el.removeEventListener("keydown", handler);
    },
  };
}
