/** The selected tab's scrolling body. */
const BODY = '[data-nav-side] [role="tabpanel"]:not([hidden]) .preview__body';
/** The empty pane's first button, and the strip's "New tab" button: where focus goes when there is no body. */
const EMPTY_FIRST = "[data-nav-side] .empty-pane__button";
const PLUS = '[data-nav-side] button[aria-label="New tab"]';

/** The element focus should reach when the side pane takes it, or null when the pane isn't drawn. */
export function paneFocusTarget(root: ParentNode = document): HTMLElement | null {
  return root.querySelector<HTMLElement>(BODY) ?? root.querySelector<HTMLElement>(EMPTY_FIRST) ?? root.querySelector<HTMLElement>(PLUS);
}

/**
 * Moves focus into the side pane after the next frame (a new tab or a pane that is just showing needs a frame to be there):
 * the selected tab's body, else the empty pane's first button, else the strip's "New tab" button. Opening is the opener's job;
 * mounting or selecting a tab never takes focus, so a session switch leaves it in the terminal.
 */
export function focusPane(): void {
  let frames = 0;
  const attempt = () => {
    const target = paneFocusTarget();
    if (target) return void target.focus({ preventScroll: true });
    // The pane may take a second frame to be laid out when the same click shows it.
    if (++frames < 3) requestAnimationFrame(attempt);
  };
  requestAnimationFrame(attempt);
}
