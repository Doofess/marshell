import { browser } from "../browser/browserApi";
import { entryReason, quietly } from "../browser/pageFocus";

/** The selected tab's scrolling body. */
const BODY = '[data-nav-side] [role="tabpanel"]:not([hidden]) .preview__body';
/** The empty pane's first button, and the strip's "New tab" button: where focus goes when there is no body. */
/** A browser tab's slot: the tab has no body to scroll. Focusing the slot enters the page (BrowserTab). */
const SLOT = '[data-nav-side] [role="tabpanel"]:not([hidden]) .browser-tab__slot';
const EMPTY_FIRST = "[data-nav-side] .empty-pane__button";
const PLUS = '[data-nav-side] button[aria-label="New tab"]';

/** The element focus should reach when the side pane takes it: the selected tab's body or a browser tab's slot, else the empty pane's first button, else the strip's "New tab" button. Null when the pane isn't drawn. */
export function paneFocusTarget(root: ParentNode = document): HTMLElement | null {
  return root.querySelector<HTMLElement>(BODY) ?? root.querySelector<HTMLElement>(SLOT) ?? root.querySelector<HTMLElement>(EMPTY_FIRST) ?? root.querySelector<HTMLElement>(PLUS);
}

/**
 * Moves focus into the side pane after the next frame (a new tab or a pane that is just showing needs a frame to be there):
 * the selected tab's body (a browser tab's slot, which enters its page), else the empty pane's first button, else the strip's "New tab" button. Opening is the opener's job;
 * mounting or selecting a tab never takes focus, so a session switch leaves it in the terminal.
 */
export function focusPane(): void {
  let frames = 0;
  const attempt = () => {
    const target = paneFocusTarget();
    if (target) {
      const page = target.dataset.pageKey;
      if (page === undefined) return void target.focus({ preventScroll: true });
      // A browser tab's slot: the page takes the keyboard, wherever focus was and whether or not the slot already had it.
      quietly(() => target.focus({ preventScroll: true }));
      return void browser.focus(page, entryReason(page)).catch(() => {});
    }
    // The pane may take a second frame to be laid out when the same click shows it.
    if (++frames < 3) requestAnimationFrame(attempt);
  };
  requestAnimationFrame(attempt);
}
