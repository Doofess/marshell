import { useEffect, useEffectEvent, useImperativeHandle, useLayoutEffect, useRef, useState, type FocusEvent, type HTMLAttributes, type PointerEvent as ReactPointerEvent, type ReactNode, type Ref } from "react";
import { cropImage } from "../regions/crop";
import { placePopover } from "../regions/placement";
import { RegionComment, type RegionTarget } from "../regions/RegionComment";
import { RegionLayer } from "../regions/RegionLayer";
import { fromDrag, pageRegionLabel, type Region } from "../regions/region";
import { BLANK } from "./address";
import { browser, type NavAction } from "./browserApi";
import { useCoverPage, usePageCovered } from "./pageCover";
import { enterReason, isQuiet, quietly } from "./pageFocus";
import { pageRegionStep, type PageRegion, type PageRegionEvent } from "./pageRegion";
import { openPage, useFocusedPage, usePage, type PageState } from "./pageStore";
import { NO_PICTURE_TEXT, UrlBar, type UrlBarHandle } from "./UrlBar";
import { usePageSlot } from "./usePageSlot";
import "./BrowserTab.css";

/**
 * What the slot shows while the page is frozen: the picture, or null when the capture failed. `phase` says how far the freeze is: the
 * picture of a `capturing` freeze may be an earlier one, and only a `frozen` one is the page as it was when the cover went up.
 */
export type Frozen = { picture: string | null; phase: "capturing" | "frozen" | "thawing" };

/** A region being marked on the page: what the slot draws over the picture, and what its keys and pointer do. */
export type RegionView = {
  step: "wait" | "box" | "comment";
  region: Region;
  busy: boolean;
  error: string | null;
  /** Where the crop goes, for the comment box. */
  target: RegionTarget | null;
  onChange(r: Region): void;
  onConfirm(): void;
  onCancel(): void;
  onBack(): void;
  onAdd(comment: string): void;
  /** The pointer handlers of the surface that takes a drag. */
  onDrag: Pick<HTMLAttributes<HTMLDivElement>, "onPointerDown" | "onPointerMove" | "onPointerUp" | "onDoubleClick">;
};

export type BrowserTabViewProps = {
  page: PageState | undefined;
  /** The tab's saved address. */
  url: string;
  /** False where the shell has no browser tabs. */
  available: boolean;
  frozen: Frozen | null;
  /** The page has the keyboard: the slot draws its focus ring in the gutter around the page. */
  keys: boolean;
  slotRef: Ref<HTMLDivElement>;
  barRef?: Ref<UrlBarHandle>;
  onGo(url: string): void;
  onNav(action: Exclude<NavAction, "go">): void;
  onOpenOutside(url: string): void;
  onEscape(): void;
  announce?: (text: string) => void;
  /** The bar's "Select region" button, and whether the page can be marked. */
  onRegion?: () => void;
  canRegion?: boolean;
  /** A region is being marked. */
  selection?: RegionView | null;
  /** A line under the bar, ahead of the page's own status. */
  note?: string | null;
  /** The page's key, put on the slot so the app finds the slot a page's keys and focus belong to. */
  pageKey?: string;
  /** Focus reached the slot. */
  onSlotFocus?: (e: FocusEvent<HTMLDivElement>) => void;
};

/** The comment box beside the box, below or above it, kept inside the slot, and focused once it is placed. */
function PagePopover({ region, children }: { region: Region; children: ReactNode }) {
  const holder = useRef<HTMLDivElement>(null);
  const [at, setAt] = useState<{ left: number; top: number } | null>(null);
  useLayoutEffect(() => {
    const pop = holder.current;
    const layer = pop?.parentElement;
    const box = layer?.querySelector<HTMLElement>(".region-box");
    const view = pop?.closest<HTMLElement>(".browser-tab__slot");
    if (!pop || !layer || !box || !view) return;
    const place = () => {
      const b = box.getBoundingClientRect();
      const v = view.getBoundingClientRect();
      const p = placePopover({ left: b.left, top: b.top, width: b.width, height: b.height }, { w: pop.offsetWidth, h: pop.offsetHeight }, { left: v.left, top: v.top, width: v.width, height: v.height });
      setAt((prev) => (prev && prev.left === p.left && prev.top === p.top ? prev : { left: p.left, top: p.top }));
    };
    place();
    window.addEventListener("resize", place);
    // The slot, the box and the comment itself change size without the window moving (a split dragged, an error line appearing).
    const watch = typeof ResizeObserver === "undefined" ? null : new ResizeObserver(place);
    for (const el of [view, box, layer, pop]) watch?.observe(el);
    return () => {
      window.removeEventListener("resize", place);
      watch?.disconnect();
    };
  }, [region]);
  // A hidden input cannot take focus, so focus moves into the comment once it shows.
  const focused = useRef(false);
  useEffect(() => {
    if (!at || focused.current) return;
    focused.current = true;
    holder.current?.querySelector<HTMLInputElement>("input")?.focus({ preventScroll: true });
  }, [at]);
  return (
    <div ref={holder} className="region-popover" style={at ? { left: at.left, top: at.top } : { left: 0, top: 0, visibility: "hidden" }}>
      {children}
    </div>
  );
}

/** The tab's markup: the bar, its status line, and the slot the shell's page sits over. Without browser tabs, a plain explanation. */
export function BrowserTabView({ page, url, available, frozen, keys, slotRef, barRef, onGo, onNav, onOpenOutside, onEscape, announce, onRegion, canRegion, selection, note, pageKey, onSlotFocus }: BrowserTabViewProps) {
  if (!available) {
    const address = page?.url || url;
    return (
      <div className="browser-tab browser-tab--unavailable">
        <p className="browser-tab__note">Browser tabs aren't available on this system yet.</p>
        <button type="button" className="browser-tab__button" disabled={address === "" || address === BLANK} onClick={() => onOpenOutside(address)}>
          Open in the system browser
        </button>
      </div>
    );
  }
  return (
    <div className="browser-tab">
      <UrlBar
        ref={barRef}
        page={page}
        fallbackUrl={url}
        onGo={onGo}
        onNav={onNav}
        onOpenOutside={onOpenOutside}
        onEscape={onEscape}
        onRegion={onRegion}
        canRegion={canRegion}
        selecting={selection != null}
        note={note}
        announce={announce}
      />
      <div ref={slotRef} className="browser-tab__slot" tabIndex={selection != null ? -1 : 0} aria-label="Web page" data-page-key={pageKey} data-keys={keys ? "" : undefined} onFocus={onSlotFocus}>
        {frozen !== null && (frozen.picture !== null ? <img alt="" className="browser-tab__picture" src={frozen.picture} /> : <div className="browser-tab__fill" />)}
        {selection != null && selection.step !== "wait" && frozen?.picture != null && (
          // In the slot's one cell, over the picture and the size of it, so the box's fractions are the picture's.
          <div className="browser-tab__region">
            <div className="region-capture" {...selection.onDrag} />
            <RegionLayer region={selection.region} onChange={selection.onChange} onConfirm={selection.onConfirm} onCancel={selection.onCancel} paged={false} />
            {selection.step === "comment" && (
              <PagePopover region={selection.region}>
                <RegionComment target={selection.target} busy={selection.busy} error={selection.error} onAdd={selection.onAdd} onCancel={selection.onBack} />
              </PagePopover>
            )}
          </div>
        )}
      </div>
    </div>
  );
}

/** What a parent holds of a browser tab: its address box, and the region selection. */
export type BrowserTabHandle = UrlBarHandle & {
  /** Starts marking a region on the page. False when the tab has no live page to mark. */
  selectRegion(): boolean;
};

export type BrowserTabProps = {
  /** The page's key: the session and the pane tab (`pageKey`). */
  pageKey: string;
  /** The tab's saved address: where a page that isn't loaded opens. */
  url: string;
  /** This tab is the one on screen. */
  active: boolean;
  /** False where the shell has no browser tabs (`browserTabs`). */
  available?: boolean;
  /** The page's address and title changed: for the layout. */
  onPage(url: string, title: string): void;
  /** Open this address in the system browser. */
  onOpenOutside(url: string): void;
  /** Esc in the address box. */
  onEscape(): void;
  announce?: (text: string) => void;
  /** Where a marked region goes: the focused session's name and how it gets there, or null with no session. */
  regionTarget?: RegionTarget | null;
  /** Hands over a marked region's crop and comment. Resolves when it is placed; rejects with the reason it was not. */
  onRegion?: (r: { png: Blob; label: string; comment: string }) => Promise<void>;
  /** Focuses and selects the address box; starts a region. */
  ref?: Ref<BrowserTabHandle>;
};

const ignore = () => {};

type GoApi = {
  nav(key: string, action: "go", url: string): Promise<unknown>;
  open(key: string, url: string): Promise<unknown>;
  focus(key: string, reason: "programmatic"): Promise<unknown>;
};

/**
 * Enter in the address box: the page goes to the address (a page that is gone opens there), and the keyboard follows it into the
 * page, as in any browser. It stays in the box when the shell refuses the address, and wherever the person moved it meanwhile.
 */
export async function goTo(api: GoApi, key: string, exists: boolean, to: string, stillInBar: () => boolean, focusSlot: () => void): Promise<void> {
  try {
    await (exists ? api.nav(key, "go", to) : api.open(key, to));
  } catch {
    return;
  }
  if (!stillInBar()) return;
  focusSlot();
  await api.focus(key, "programmatic").catch(ignore);
}

/** A line under the bar, with the page's address and status as they were when it was set. */
export type PageNote = { text: string; url: string; status: PageState["status"] };

/** The note while the page's address and status are still those it was set for; the next page event makes it stale. */
export function noteFor(note: PageNote | null, url: string, status: PageState["status"]): string | null {
  return note !== null && note.url === url && note.status === status ? note.text : null;
}

/** A browser tab: the URL bar over a slot, and the page the shell keeps there while the tab is on screen. */
export function BrowserTab({ pageKey, url, active, available = true, onPage, onOpenOutside, onEscape, announce, regionTarget = null, onRegion, ref }: BrowserTabProps) {
  const page = usePage(pageKey);
  const slot = useRef<HTMLDivElement>(null);
  const bar = useRef<UrlBarHandle>(null);
  const covered = usePageCovered();
  const frozen = usePageSlot(slot, pageKey, active && available, covered, url);
  const keys = useFocusedPage() === pageKey;

  const say = useEffectEvent((u: string, t: string) => onPage(u, t));
  const pageUrl = page?.url ?? "";
  const pageTitle = page?.title ?? "";
  useEffect(() => {
    if (pageUrl !== "") say(pageUrl, pageTitle);
  }, [pageUrl, pageTitle]);

  // A region marked on the page. The selection works on the picture the freeze takes while the selection holds a cover, and on nothing
  // else: no script runs in the page. The picture is pinned when the box comes up, and the crop is made from that one.
  const [sel, setSel] = useState<PageRegion>(null);
  const [note, setNote] = useState<PageNote | null>(null);
  const pinned = useRef<string | null>(null);
  const latest = useRef({ title: pageTitle, url: pageUrl });
  latest.current = { title: pageTitle, url: pageUrl };
  const selecting = sel !== null;
  useCoverPage(selecting);
  const send = (e: PageRegionEvent) => setSel((s) => pageRegionStep(s, e));
  // Ends the selection. Focus goes back to the "Select region" button unless the keyboard is somewhere else by now or the tab went away.
  const end = (e: PageRegionEvent, focus: boolean) => {
    const el = document.activeElement;
    const here = el === null || el === document.body || slot.current?.contains(el) === true;
    send(e);
    if (focus && here) bar.current?.focusRegion();
  };
  const phase = frozen?.phase;
  const picture = frozen?.picture ?? null;
  const step = sel?.step;
  const watchPicture = useEffectEvent(() => {
    if (step === "wait") {
      if (phase !== "frozen") return;
      if (picture === null) {
        // The capture failed: there is no box, and the bar says why.
        setNote({ text: NO_PICTURE_TEXT, url: pageUrl, status: page?.status ?? null });
        end({ type: "picture", ok: false }, true);
      } else {
        pinned.current = picture;
        send({ type: "picture", ok: true });
      }
    } else if (step !== undefined && (phase !== "frozen" || picture !== pinned.current)) {
      // The picture went away or is another one: nothing to crop from.
      end({ type: "cancel" }, true);
    }
    if (step === undefined) pinned.current = null;
  });
  useEffect(() => watchPicture(), [step, phase, picture]);
  // Another address, a page that is gone, or the tab going off screen ends it.
  const watchPage = useEffectEvent(() => {
    if (sel === null) return;
    if (!active || !available || page?.live !== true) end({ type: "hidden" }, false);
    else if (pageRegionStep(sel, { type: "moved", url: pageUrl }) === null) end({ type: "moved", url: pageUrl }, true);
  });
  useEffect(() => watchPage(), [pageUrl, active, available, page?.live]);
  // The side pane folding leaves the slot with no size.
  useEffect(() => {
    const el = slot.current;
    if (!selecting || !el || typeof ResizeObserver === "undefined") return;
    const watch = new ResizeObserver(() => {
      if (el.clientWidth === 0 || el.clientHeight === 0) end({ type: "hidden" }, false);
    });
    watch.observe(el);
    return () => watch.disconnect();
  }, [selecting]);
  // The box takes focus when it is shown, and again after a drag.
  const [focusBox, setFocusBox] = useState(0);
  useEffect(() => {
    if (step === "box") slot.current?.querySelector<HTMLElement>(".region-box")?.focus({ preventScroll: true });
  }, [step, focusBox]);

  const canRegion = available && active && page?.live === true && pageUrl !== "" && pageUrl !== BLANK;
  const start = (): boolean => {
    if (!canRegion) return false;
    if (sel !== null) return true;
    setNote(null);
    send({ type: "start", url: pageUrl });
    return true;
  };
  useImperativeHandle(ref, () => ({ focusAddress: () => bar.current?.focusAddress(), focusRegion: () => bar.current?.focusRegion(), selectRegion: start }));
  // The label is the page's title and address as they are at the moment of Enter.
  const add = async (comment: string) => {
    const s = sel;
    const from = pinned.current;
    if (s === null || s.step !== "comment" || s.busy || from === null || !onRegion) return;
    const { title, url: address } = latest.current;
    if (address !== s.url) return end({ type: "moved", url: address }, true);
    send({ type: "busy" });
    try {
      const png = await cropImage(from, s.region);
      await onRegion({ png, label: pageRegionLabel(title, address), comment });
      end({ type: "added" }, true);
    } catch (e) {
      send({ type: "failed", error: e instanceof Error ? e.message : String(e) });
    }
  };

  const drag = useRef<{ from: { x: number; y: number } } | null>(null);
  const view: RegionView | null = sel && {
    step: sel.step,
    region: sel.region,
    busy: sel.busy,
    error: sel.error,
    target: regionTarget,
    onChange: (region) => setSel((s) => s && { ...s, region }),
    onConfirm: () => send({ type: "confirm" }),
    onCancel: () => end({ type: "cancel" }, true),
    onBack: () => {
      send({ type: "back" });
      setFocusBox((n) => n + 1);
    },
    onAdd: (c) => void add(c),
    onDrag: {
      onPointerDown: (e: ReactPointerEvent<HTMLDivElement>) => {
        if (e.button !== 0) return;
        const r = e.currentTarget.getBoundingClientRect();
        drag.current = { from: { x: e.clientX - r.left, y: e.clientY - r.top } };
        e.currentTarget.setPointerCapture(e.pointerId);
        e.preventDefault();
      },
      onPointerMove: (e: ReactPointerEvent<HTMLDivElement>) => {
        const d = drag.current;
        if (!d) return;
        const r = e.currentTarget.getBoundingClientRect();
        const next = fromDrag(1, d.from, { x: e.clientX - r.left, y: e.clientY - r.top }, { w: r.width, h: r.height });
        if (next) setSel((s) => s && { ...s, region: next, step: "box" });
      },
      onPointerUp: (e: ReactPointerEvent<HTMLDivElement>) => {
        if (!drag.current) return;
        drag.current = null;
        if (e.currentTarget.hasPointerCapture(e.pointerId)) e.currentTarget.releasePointerCapture(e.pointerId);
        setFocusBox((n) => n + 1);
      },
      onDoubleClick: () => send({ type: "confirm" }),
    },
  };

  const go = (to: string) => {
    const tab = slot.current?.parentElement;
    const stillInBar = () => document.activeElement instanceof HTMLInputElement && tab?.contains(document.activeElement) === true;
    // The slot takes focus quietly: the page is told to take the keyboard right after, and must not be entered twice.
    // A page the shell doesn't have (never made, refused, unloaded) opens at the address; a page it has goes there.
    const api = { nav: browser.nav, open: openPage, focus: browser.focus };
    void goTo(api, pageKey, page?.live === true, to, stillInBar, () => quietly(() => slot.current?.focus({ preventScroll: true })));
  };
  // Tab reached the slot from an app element: the page starts at its first element, or its last when Shift+Tab came from after it
  // (S12 finding 9). Focus arriving with no element before it is the window coming back, which useBrowserKeys's handback deals with;
  // a key the app took from the page puts focus here quietly. Focus on the region's box or comment, which sit inside the slot, is not
  // the page's: the page is hidden behind its picture then.
  const enter = (e: FocusEvent<HTMLDivElement>) => {
    const from = e.relatedTarget;
    if (!available || sel !== null || e.target !== e.currentTarget || isQuiet() || !(from instanceof Element)) return;
    const slotIsBefore = (e.currentTarget.compareDocumentPosition(from) & Node.DOCUMENT_POSITION_FOLLOWING) !== 0;
    browser.focus(pageKey, enterReason(slotIsBefore)).catch(ignore);
  };
  return (
    <BrowserTabView
      pageKey={pageKey}
      onSlotFocus={enter}
      page={page}
      url={url}
      available={available}
      frozen={frozen}
      keys={keys}
      slotRef={slot}
      barRef={bar}
      onGo={go}
      onNav={(action) => void browser.nav(pageKey, action).catch(ignore)}
      onOpenOutside={onOpenOutside}
      onEscape={onEscape}
      announce={announce}
      onRegion={onRegion ? () => (sel ? end({ type: "cancel" }, true) : void start()) : undefined}
      canRegion={canRegion}
      selection={view}
      note={noteFor(note, pageUrl, page?.status ?? null)}
    />
  );
}
