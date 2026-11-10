import { useEffect, useEffectEvent, useRef, type FocusEvent, type Ref } from "react";
import { BLANK } from "./address";
import { browser, type NavAction } from "./browserApi";
import { usePageCovered } from "./pageCover";
import { enterReason, isQuiet, quietly } from "./pageFocus";
import { openPage, usePage, type PageState } from "./pageStore";
import { UrlBar, type UrlBarHandle } from "./UrlBar";
import { usePageSlot } from "./usePageSlot";
import "./BrowserTab.css";

/** What the slot shows while the page is frozen: the picture, or null when the capture failed. */
export type Frozen = { picture: string | null };

export type BrowserTabViewProps = {
  page: PageState | undefined;
  /** The tab's saved address. */
  url: string;
  /** False where the shell has no browser tabs. */
  available: boolean;
  frozen: Frozen | null;
  slotRef: Ref<HTMLDivElement>;
  barRef?: Ref<UrlBarHandle>;
  onGo(url: string): void;
  onNav(action: Exclude<NavAction, "go">): void;
  onOpenOutside(url: string): void;
  onEscape(): void;
  announce?: (text: string) => void;
  /** The page's key, put on the slot so the app finds the slot a page's keys and focus belong to. */
  pageKey?: string;
  /** Focus reached the slot. */
  onSlotFocus?: (e: FocusEvent<HTMLDivElement>) => void;
};

/** The tab's markup: the bar, its status line, and the slot the shell's page sits over. Without browser tabs, a plain explanation. */
export function BrowserTabView({ page, url, available, frozen, slotRef, barRef, onGo, onNav, onOpenOutside, onEscape, announce, pageKey, onSlotFocus }: BrowserTabViewProps) {
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
      <UrlBar ref={barRef} page={page} fallbackUrl={url} onGo={onGo} onNav={onNav} onOpenOutside={onOpenOutside} onEscape={onEscape} announce={announce} />
      <div ref={slotRef} className="browser-tab__slot" tabIndex={0} aria-label="Web page" data-page-key={pageKey} onFocus={onSlotFocus}>
        {frozen !== null && (frozen.picture !== null ? <img alt="" className="browser-tab__picture" src={frozen.picture} /> : <div className="browser-tab__fill" />)}
      </div>
    </div>
  );
}

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
  /** Focuses and selects the address box. */
  ref?: Ref<UrlBarHandle>;
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

/** A browser tab: the URL bar over a slot, and the page the shell keeps there while the tab is on screen. */
export function BrowserTab({ pageKey, url, active, available = true, onPage, onOpenOutside, onEscape, announce, ref }: BrowserTabProps) {
  const page = usePage(pageKey);
  const slot = useRef<HTMLDivElement>(null);
  const covered = usePageCovered();
  const frozen = usePageSlot(slot, pageKey, active && available, covered, url);

  const say = useEffectEvent((u: string, t: string) => onPage(u, t));
  const pageUrl = page?.url ?? "";
  const pageTitle = page?.title ?? "";
  useEffect(() => {
    if (pageUrl !== "") say(pageUrl, pageTitle);
  }, [pageUrl, pageTitle]);

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
  // a key the app took from the page puts focus here quietly.
  const enter = (e: FocusEvent<HTMLDivElement>) => {
    const from = e.relatedTarget;
    if (!available || isQuiet() || !(from instanceof Element)) return;
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
      slotRef={slot}
      barRef={ref}
      onGo={go}
      onNav={(action) => void browser.nav(pageKey, action).catch(ignore)}
      onOpenOutside={onOpenOutside}
      onEscape={onEscape}
      announce={announce}
    />
  );
}
