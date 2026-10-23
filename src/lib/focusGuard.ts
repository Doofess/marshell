import { FOCUSABLE } from "./dialogFocus";

/** What the guard needs of the popup's element: whether a node is inside it, and whether it is still on the page. */
export type GuardNode = { contains(n: unknown): boolean; isConnected: boolean };

/** The little of the page the guard touches, so it can run against a fake one in tests. */
export type GuardDom = {
  /** The focused element, or null when nothing is (the body counts as nothing). */
  active(): unknown;
  /** True for a node inside another dialog or menu than this popup (one that opened over it, or one the guard doesn't know). */
  inOtherLayer(target: unknown): boolean;
  /** Listens for focus arriving anywhere in the page; returns the way to stop. */
  onFocusIn(handler: (target: unknown) => void): () => void;
  /** Listens for focus leaving the popup, with where it went (null: nowhere); returns the way to stop. */
  onFocusOut(handler: (related: unknown) => void): () => void;
  /** Listens for the window getting the focus back; returns the way to stop. */
  onWindowFocus(handler: () => void): () => void;
  /** Runs `f` on the next frame; returns the way to cancel it. */
  later(f: () => void): () => void;
};

type Focusable = { focus(): void; isConnected?: boolean };

/** The open guards, newest last. Only the newest acts, so two open popups never pull the focus between them. */
const OPEN: object[] = [];

/**
 * One open of a popup (a menu, a sheet, a dialog): while it is open the focus stays inside it. Focus that lands outside (a
 * terminal focused by a timer, the page after Tab, nothing after the window lost focus) comes back on the next frame to the
 * last control it held, or to `fallback()`. Focus that moves into another dialog or menu is left there, and a popup that has
 * left the page by then (a close that moved the focus on, like Rename's box) is not reclaimed. The returned function ends it.
 */
export function holdFocus(popup: GuardNode, dom: GuardDom, fallback: () => Focusable | null, open: object[] = OPEN): () => void {
  const me = {};
  open.push(me);
  let last: Focusable | null = null;
  let cancel: (() => void) | null = null;
  const inside = (n: unknown) => n !== null && popup.contains(n);
  const check = () => {
    cancel = null;
    if (open[open.length - 1] !== me || !popup.isConnected) return;
    const a = dom.active();
    if (inside(a) || (a !== null && dom.inOtherLayer(a))) return;
    (last?.isConnected !== false && last ? last : fallback())?.focus();
  };
  const soon = () => {
    cancel?.();
    cancel = dom.later(check);
  };
  const stops = [
    dom.onFocusIn((t) => {
      if (inside(t)) last = t as Focusable;
      else soon();
    }),
    dom.onFocusOut((r) => r === null && soon()),
    dom.onWindowFocus(soon),
  ];
  return () => {
    cancel?.();
    stops.forEach((s) => s());
    open.splice(open.indexOf(me), 1);
  };
}

/** The controls a popup's Tab walks, in order: shown, and in the tab order (a roving radio's other options are left out). */
export function tabbable<T extends { tabIndex: number }>(nodes: T[], shown: (n: T) => boolean): T[] {
  return nodes.filter((n) => n.tabIndex >= 0 && shown(n));
}

/** A popup's or dialog's role, and a modal's flag: focus inside one of these that isn't ours belongs to that layer. */
const LAYER = '[role="dialog"], [role="alertdialog"], [role="menu"], [aria-modal="true"]';

/** The real page behind a popup element. */
export function guardDom(el: HTMLElement, doc: Document, win: Window): GuardDom {
  return {
    active: () => (doc.activeElement && doc.activeElement !== doc.body ? doc.activeElement : null),
    inOtherLayer: (t) => {
      const layer = t instanceof Element ? t.closest(LAYER) : null;
      return layer !== null && layer !== el && !layer.contains(el) && !el.contains(layer);
    },
    onFocusIn: (h) => {
      const f = (e: FocusEvent) => h(e.target);
      doc.addEventListener("focusin", f, true);
      return () => doc.removeEventListener("focusin", f, true);
    },
    onFocusOut: (h) => {
      const f = (e: FocusEvent) => {
        if (!(e.relatedTarget instanceof Node && el.contains(e.relatedTarget))) h(e.relatedTarget);
      };
      el.addEventListener("focusout", f);
      return () => el.removeEventListener("focusout", f);
    },
    onWindowFocus: (h) => {
      win.addEventListener("focus", h);
      return () => win.removeEventListener("focus", h);
    },
    later: (f) => {
      const id = win.requestAnimationFrame(f);
      return () => win.cancelAnimationFrame(id);
    },
  };
}

/** The tabbable controls inside a popup element, in document order. */
export function tabbableIn(el: HTMLElement): HTMLElement[] {
  return tabbable([...el.querySelectorAll<HTMLElement>(FOCUSABLE)], (n) => n.getClientRects().length > 0);
}
