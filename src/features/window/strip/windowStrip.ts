// The strip over the window's buttons (PLAN §9, window-buttons strip decisions). The page paints the buttons' pictures from its
// own colours and sends them to the shell; this keeps what the shell has equal to what the page looks like now.
import { invoke } from "@tauri-apps/api/core";
import { inTauri } from "../../../lib/window/appWindow";
import { pageCovered, subscribeCover } from "../../browser/pageCover";
import { canvasSurface, paintStrip, readLooks, tileLayout, type StripPaint } from "./tiles";

export type StripRead = { shown: boolean; key: string | null; paint(): StripPaint | null };

/** The strip shows only while nothing covers the page and the keyboard isn't on a button (the page's own focus ring is then the one to see). */
export function stripShown(covered: boolean, keyboardOnButtons: boolean): boolean {
  return !covered && !keyboardOnButtons;
}

/**
 * Returns a function that reads the page and sends when `shown` or `key` differs from the last send. `paint()` is called only
 * when `key` changed, since the pictures are large. A send that fails is forgotten whole, so the next call sends again. A call
 * while a send is in flight waits for it, then sends whatever the page is by then.
 */
export function stripSync(read: () => StripRead, send: (shown: boolean, paint: StripPaint | null) => Promise<void>): () => void {
  let last: { shown: boolean; key: string | null } | null = null;
  let inFlight = false;
  let again = false;
  const sync = (): void => {
    if (inFlight) {
      again = true;
      return;
    }
    const now = read();
    const key = now.key;
    if (last && last.shown === now.shown && last.key === key) return;
    const keyChanged = key !== null && last?.key !== key;
    // With no key, or no pictures to go with it, the strip hides and the key isn't remembered, so the next call tries again.
    const paint = keyChanged ? now.paint() : null;
    const hidden = key === null || (keyChanged && paint === null);
    const sent = hidden ? { shown: false, key: null } : { shown: now.shown, key };
    last = sent;
    inFlight = true;
    send(sent.shown, paint).then(
      () => settled(),
      () => {
        if (last === sent) last = null;
        settled();
      },
    );
  };
  const settled = (): void => {
    inFlight = false;
    if (again) {
      again = false;
      sync();
    }
  };
  return sync;
}

/**
 * The listeners for the keyboard coming to and leaving the three buttons. A move from one button to the next fires `focusout` and
 * then `focusin`; a read between the two finds the keyboard on no button and would show the strip over the next button's ring for
 * a moment. So the leaving half waits a turn (`defer` returns its cancel), and the arriving half reads at once.
 */
export function focusWatch(sync: () => void, defer: (run: () => void) => () => void): { onIn(): void; onOut(): void; stop(): void } {
  let cancel: (() => void) | null = null;
  const stop = () => {
    cancel?.();
    cancel = null;
  };
  return {
    onIn: () => sync(),
    onOut: () => {
      stop();
      cancel = defer(() => {
        cancel = null;
        sync();
      });
    },
    stop,
  };
}

/** Keeps the strip's pictures and visibility equal to the page's, now and on every change that can alter them. Returns the stop function. */
export function watchWindowStrip(controls: HTMLElement, doc: Document = document): () => void {
  const view = doc.defaultView;
  if (!inTauri() || !view) return () => {};
  const send = (shown: boolean, paint: StripPaint | null) =>
    invoke<void>("set_window_strip", { shown, paint }).catch((error: unknown) => {
      // Not silent: until the next change the strip keeps its old pictures, or none.
      console.error("Marshell: the window buttons' strip couldn't be updated.", error);
      throw error;
    });
  const sync = stripSync(() => {
    const looks = readLooks(controls);
    return {
      shown: stripShown(pageCovered(), controls.querySelector(":focus-visible") !== null),
      key: looks ? JSON.stringify([view.devicePixelRatio, looks]) : null,
      paint: () => (looks ? paintStrip(tileLayout(view.devicePixelRatio), looks, canvasSurface(doc)) : null),
    };
  }, send);
  const onChange = () => sync();
  const queries = ["(prefers-color-scheme: dark)", "(forced-colors: active)", "(prefers-contrast: more)"].map((q) => view.matchMedia(q));
  for (const q of queries) q.addEventListener("change", onChange);
  const attributes = new MutationObserver(onChange);
  attributes.observe(doc.documentElement, { attributes: true });
  const unsubscribeCover = subscribeCover(onChange);
  const focus = focusWatch(sync, (run) => {
    const timer = view.setTimeout(run, 0);
    return () => view.clearTimeout(timer);
  });
  controls.addEventListener("focusin", focus.onIn);
  controls.addEventListener("focusout", focus.onOut);
  // A query for the current scale fires once, when the scale leaves it; the next one is made for the new scale.
  let scale: MediaQueryList | null = null;
  const onScale = () => {
    scale?.removeEventListener("change", onScale);
    sync();
    scale = view.matchMedia(`(resolution: ${view.devicePixelRatio}dppx)`);
    scale.addEventListener("change", onScale);
  };
  scale = view.matchMedia(`(resolution: ${view.devicePixelRatio}dppx)`);
  scale.addEventListener("change", onScale);
  sync();
  return () => {
    for (const q of queries) q.removeEventListener("change", onChange);
    scale?.removeEventListener("change", onScale);
    attributes.disconnect();
    unsubscribeCover();
    controls.removeEventListener("focusin", focus.onIn);
    controls.removeEventListener("focusout", focus.onOut);
    focus.stop();
    invoke<void>("set_window_strip", { shown: false, paint: null }).catch(() => {});
  };
}
