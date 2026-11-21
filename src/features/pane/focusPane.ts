import { isPanelId, type PanelId } from "./panels";
import { browser } from "../browser/browserApi";
import { entryReason, quietly } from "../browser/pageFocus";

const nav = (panel: PanelId) => `[data-nav-side="${panel}"]`;
/** A text tab's editor, when it is the one on screen: not behind the Rendered view, and not in a hidden tab panel. */
const editor = (panel: PanelId) => `${nav(panel)} [role="tabpanel"]:not([hidden]) [data-pane="editor"]:not([hidden]) .cm-content`;
/** The selected tab's scrolling body. */
const body = (panel: PanelId) => `${nav(panel)} [role="tabpanel"]:not([hidden]) .preview__body`;
/** A browser tab's slot: the tab has no body to scroll. Focusing the slot enters the page (BrowserTab). */
const slot = (panel: PanelId) => `${nav(panel)} [role="tabpanel"]:not([hidden]) .browser-tab__slot`;
/** The empty panel's first usable button, and the strip's + button: where focus goes when there is no body. */
const emptyFirst = (panel: PanelId) => `${nav(panel)} .empty-panel__button:not([aria-disabled="true"])`;
const plusOf = (panel: PanelId) => `${nav(panel)} button[data-pane-plus]`;
/** The selected tab in the strip. */
const selectedTab = (panel: PanelId) => `${nav(panel)} .side-pane__strip [role="tab"][aria-selected="true"]`;

/** The panel an element is inside, or null. */
export function panelOfElement(el: Element | null): PanelId | null {
  const v = el?.closest("[data-nav-side]")?.getAttribute("data-nav-side");
  return v != null && isPanelId(v) ? v : null;
}

/** Where in the side pane the keyboard is: on the tab strip, elsewhere in the pane, or not in the pane at all. */
export function askedFrom(el: Element | null): "strip" | "pane" | null {
  if (!el) return null;
  if (el.closest(".side-pane__strip")) return "strip";
  return el.closest("[data-nav-side]") ? "pane" : null;
}

/** The element focus should reach when a panel takes it: a text tab's editor (so typing goes on where the cursor was), else the selected tab's body or a browser tab's slot, else the empty pane's first button, else the strip's "New tab" button. Null when the panel isn't drawn. */
export function paneFocusTarget(root: ParentNode, panel: PanelId, closing?: string, where: "pane" | "strip" = "pane"): HTMLElement | null {
  // A tab that is closing is still drawn: its editor would take the focus and go away with it.
  if (closing !== undefined && root.querySelector(`${nav(panel)} [role="tab"][id="pane-tab-${closing}"]`)) return null;
  // A close asked from the strip leaves the keyboard on the strip, as a close that asks nothing does: the next Delete closes the
  // next tab, and types nothing into its file.
  if (where === "strip") return root.querySelector<HTMLElement>(selectedTab(panel)) ?? root.querySelector<HTMLElement>(plusOf(panel));
  return root.querySelector<HTMLElement>(editor(panel)) ?? root.querySelector<HTMLElement>(body(panel)) ?? root.querySelector<HTMLElement>(slot(panel)) ?? root.querySelector<HTMLElement>(emptyFirst(panel)) ?? root.querySelector<HTMLElement>(plusOf(panel));
}

/**
 * Moves focus into a panel of the side pane after the next frame (a new tab or a pane that is just showing needs a frame to be there):
 * a text tab's editor, else the selected tab's body (a browser tab's slot, which enters its page), else the empty pane's first button, else the strip's + button. Opening is the opener's job;
 * mounting or selecting a tab never takes focus, so a session switch leaves it in the terminal. `closing` is the id of a tab whose
 * close may not be drawn yet (a close that follows a save is drawn after the frame): focus waits until that tab is gone. `where`
 * is "strip" for a close that was asked from the tab strip: focus goes to the strip's selected tab instead.
 */
export function focusPane(panel: PanelId, closing?: string, where: "pane" | "strip" = "pane"): void {
  let frames = 0;
  const attempt = () => {
    const target = paneFocusTarget(document, panel, closing, where);
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
