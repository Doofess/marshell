import { useContext, useEffect, useEffectEvent, useMemo, useRef, useState, type RefObject } from "react";
import { flushSync } from "react-dom";
import { LayoutContext, paneColumn, type ColumnInputs, type Layout } from "../window/paneColumn";
import type { Frozen } from "./BrowserTab";
import { browser, type Piece, type Report } from "./browserApi";
import { createFreezer, LIVE, type Freeze } from "./freeze";
import { createPlacer } from "./placing";
import { pageRule } from "./pageRule";
import { openPage, usePage, type PageState } from "./pageStore";

/** The parts of a `DOMRect` the report reads, so a test needs no DOM. */
export type DOMRectLike = { x: number; y: number; width: number; height: number; right: number; bottom: number };

export type SlotStep = "open" | "show" | "hide" | "none";

/**
 * What the slot asks of its page. A tab that is on screen and has no page (never opened, refused, evicted, or after
 * `browser://gone`) opens it at its saved address; a page that exists is shown while its tab is on screen and nothing covers it, and hidden
 * otherwise; a tab that was never on screen has nothing to do.
 */
export function slotStep(page: PageState | undefined, active: boolean, covered: boolean): SlotStep {
  if (!page?.live) return active ? "open" : "none";
  return active && !covered ? "show" : "hide";
}

type OpenApi = {
  open(key: string, url: string): Promise<unknown>;
  focus(key: string, reason: "programmatic"): Promise<unknown>;
};

/**
 * Opens a tab's page at `url`. Focus can reach the slot before the shell has made the page (a link that opens a tab, the pane's
 * shortcut, Tab), when there is no page to take the keyboard. So a page that opens under the focused slot takes it.
 */
export async function openUnder(api: OpenApi, key: string, url: string, slotFocused: () => boolean): Promise<void> {
  try {
    await api.open(key, url);
  } catch {
    return;
  }
  if (slotFocused()) await api.focus(key, "programmatic").catch(ignore);
}

const finite = (n: number) => (Number.isFinite(n) ? n : 0);

// pageRule walks every window width up to 8192 px; its answer only changes with the column's inputs and the slot's insets,
// so the last one is kept for the reports that come once a frame while the layout moves.
let ruleKey = "";
let rulePieces: Piece[] = [];
function rulePiecesFor(inputs: ColumnInputs, start: number, end: number): Piece[] {
  const key = JSON.stringify([inputs, start, end]);
  if (key !== ruleKey) {
    rulePieces = pageRule(inputs, { start, end });
    ruleKey = key;
  }
  return rulePieces;
}

/**
 * The report for a slot's box. `slot` is in window coordinates (the window's own top left is 0, 0). The rule is the pane's column
 * at any window width, less the slot's distance from the column's two edges, with the slot's distance from the window's top and
 * foot. A slot with no size reports a zero box and no rule: there is nothing for the page to follow.
 */
export function slotReport(slot: DOMRectLike, win: DOMRectLike, inputs: ColumnInputs | null, moving: boolean): Report {
  const size = { width: finite(win.width), height: finite(win.height) };
  const x = finite(slot.x);
  const y = finite(slot.y);
  const width = finite(slot.width);
  const height = finite(slot.height);
  if (width <= 0 || height <= 0) return { rect: { x: 0, y: 0, width: 0, height: 0 }, win: size, rule: null, moving };
  const rect = { x, y, width, height };
  if (inputs === null) return { rect, win: size, rule: null, moving };
  const col = paneColumn(size.width, inputs);
  const start = x - col.x;
  const end = col.x + col.width - (x + width);
  return { rect, win: size, rule: { pieces: rulePiecesFor(inputs, start, end), top: y, bottom: size.height - (y + height) }, moving };
}

/** The slot's box in the window's coordinates, and the window's size. That is `.window`, which the app sizes itself. */
function measure(el: HTMLElement, layout: Layout | null): Report {
  const r = el.getBoundingClientRect();
  const w = el.closest<HTMLElement>(".window")?.getBoundingClientRect();
  const left = w?.left ?? 0;
  const top = w?.top ?? 0;
  const win = { x: 0, y: 0, width: w?.width ?? window.innerWidth, height: w?.height ?? window.innerHeight, right: w?.width ?? window.innerWidth, bottom: w?.height ?? window.innerHeight };
  const slot = { x: r.left - left, y: r.top - top, width: r.width, height: r.height, right: r.right - left, bottom: r.bottom - top };
  return slotReport(slot, win, layout?.inputs ?? null, layout?.moving ?? false);
}

/** Resolves after the next frame has been painted: a timeout queued from the frame's callback runs after that paint (S12). */
function nextPaint(): Promise<void> {
  return new Promise((done) => requestAnimationFrame(() => setTimeout(done, 0)));
}

// The key of the page the shell is showing, as this app last asked. A tab that goes away hides its page only if that page is
// still the one shown, so a tab that took over in the same commit isn't hidden by the one before it.
let shownKey: string | null = null;

const ignore = () => {};

/**
 * Keeps a browser tab's page over its slot. It opens the page when the tab first shows (and again when the page is gone), shows
 * it while the tab is on screen and nothing covers it, and reports the slot's box to the shell. It never closes the page.
 * `url` is the address a missing page opens at. The window's `resize` event is never a trigger: it fires a frame ahead of the
 * layout, which `.window` follows from its own observer (S12 finding 17).
 */
export function usePageSlot(ref: RefObject<HTMLElement | null>, pageKey: string, active: boolean, covered: boolean, url: string): Frozen | null {
  const page = usePage(pageKey);
  const layout = useContext(LayoutContext);
  const step = slotStep(page, active, covered);
  const placer = useRef<ReturnType<typeof createPlacer>>(null);
  placer.current ??= createPlacer((r) => browser.place(r), nextPaint);
  const savedUrl = useEffectEvent(() => url);
  const coveredNow = useEffectEvent(() => covered);

  const needOpen = step === "open";
  useEffect(() => {
    if (!needOpen) return;
    // The app's document has focus and it is on this slot: a window in the background hands nothing over.
    const slotFocused = () => ref.current !== null && document.hasFocus() && document.activeElement === ref.current;
    void openUnder({ open: openPage, focus: browser.focus }, pageKey, savedUrl(), slotFocused);
  }, [needOpen, pageKey, ref]);

  // A cover freezes the page as a picture: capture, put the picture on screen, wait for its paint, then hide the page; an
  // uncover shows the page first and drops the picture after the next paint (S12). Only a tab that is on screen with a page
  // loaded has anything to capture; any other is simply hidden, as it always was.
  const [freeze, setFreeze] = useState<Freeze>(LIVE);
  const freezer = useMemo(
    () =>
      createFreezer({
        capture: () => browser.capture(pageKey),
        show: () => {
          shownKey = pageKey;
          return browser.show(pageKey);
        },
        hide: () => {
          // Another tab may have taken the screen since the freeze began: its page is not this one's to hide.
          if (shownKey !== pageKey) return;
          shownKey = null;
          browser.show(null).catch(ignore);
        },
        paint: nextPaint,
        render: (f, now) => (now ? flushSync(() => setFreeze(f)) : setFreeze(f)),
      }),
    [pageKey],
  );
  const freezable = active && page?.live === true;
  // Leaving the freezable state (the tab goes off screen, the page goes, the key changes) puts it back to live; the page's own
  // hiding is the tab's, below.
  useEffect(() => () => freezer.reset(), [freezer, freezable]);
  useEffect(() => {
    if (freezable) freezer.send({ type: covered ? "cover" : "uncover" });
    // A page that doesn't exist yet is asked to show as soon as nothing covers it (the shell keeps that until it does), and the
    // ask is taken back when something does, so the page never opens over a menu.
    else if (active && !covered) {
      shownKey = pageKey;
      browser.show(pageKey).catch(ignore);
    } else if (active && shownKey === pageKey) {
      shownKey = null;
      browser.show(null).catch(ignore);
    }
  }, [freezer, freezable, active, covered, pageKey]);

  // The show is asked as soon as the open is: the shell keeps it until the page exists. A cover that is already up leaves it
  // hidden; the freeze above takes over from there.
  useEffect(() => {
    if (!active) return;
    if (!coveredNow()) {
      shownKey = pageKey;
      browser.show(pageKey).catch(ignore);
    }
    return () => {
      if (shownKey !== pageKey) return;
      shownKey = null;
      queueMicrotask(() => {
        if (shownKey === null) browser.show(null).catch(ignore);
      });
    };
  }, [active, pageKey]);

  const moving = layout?.moving ?? false;
  useEffect(() => {
    const el = ref.current;
    if (!el || !active) return;
    const report = () => placer.current!.report(() => measure(el, layout));
    report();
    const observer = new ResizeObserver(report);
    observer.observe(el);
    // A change of the screen's scale moves the slot's pixels without resizing it.
    let ratio: MediaQueryList | null = null;
    function onRatio() {
      report();
      watchRatio();
    }
    function watchRatio() {
      ratio?.removeEventListener("change", onRatio);
      ratio = window.matchMedia(`(resolution: ${window.devicePixelRatio}dppx)`);
      ratio.addEventListener("change", onRatio);
    }
    watchRatio();
    // While the layout moves its size doesn't change on every frame the page needs, so a report goes out each one.
    let frame = 0;
    const tick = () => {
      report();
      frame = requestAnimationFrame(tick);
    };
    if (moving) frame = requestAnimationFrame(tick);
    return () => {
      observer.disconnect();
      ratio?.removeEventListener("change", onRatio);
      cancelAnimationFrame(frame);
    };
  }, [ref, active, layout, moving]);

  return freeze.phase === "live" ? null : { picture: freeze.picture };
}
