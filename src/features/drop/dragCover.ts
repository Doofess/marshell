/**
 * How long a page's cover outlives the page's own `leave`. A page that is hidden under a drag hears a leave, and the app's
 * webview hears the enter a moment later; a drag that left the window is heard by nobody after the page's leave.
 */
export const PAGE_LEAVE_GRACE_MS = 400;

/**
 * The covers a file drag holds over a browser page. A drag that enters over a page is heard by the page's webview, so the page
 * is covered (and so frozen and hidden) from there; the app's own webview hears the rest and holds its own cover until the
 * drag leaves or drops. `cover` adds a cover and returns its release; `later` runs `fn` after `ms` and returns its cancel.
 */
export function createDragCover(cover: () => () => void, later: (fn: () => void, ms: number) => () => void) {
  let page: (() => void) | null = null;
  let app: (() => void) | null = null;
  let cancel: (() => void) | null = null;
  const stopGrace = () => {
    cancel?.();
    cancel = null;
  };
  const dropPage = () => {
    stopGrace();
    page?.();
    page = null;
  };
  const appEnd = () => {
    app?.();
    app = null;
  };
  return {
    pageEnter() {
      stopGrace();
      page ??= cover();
    },
    /** The drag left the page: for the app's webview (which takes over), or for good. */
    pageLeave() {
      if (page === null) return;
      stopGrace();
      cancel = later(dropPage, PAGE_LEAVE_GRACE_MS);
    },
    /** Files dropped on the page itself, before it was hidden. */
    pageDrop: dropPage,
    appEnter() {
      app ??= cover();
      dropPage();
    },
    /** The app's webview heard the drag leave or drop. */
    appEnd,
    dispose() {
      dropPage();
      appEnd();
    },
  };
}
