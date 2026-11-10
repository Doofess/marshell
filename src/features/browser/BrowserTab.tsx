import { useEffect, useEffectEvent, useRef, type Ref } from "react";
import { BLANK } from "./address";
import { browser, type NavAction } from "./browserApi";
import { usePageCovered } from "./pageCover";
import { usePage, type PageState } from "./pageStore";
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
};

/** The tab's markup: the bar, its status line, and the slot the shell's page sits over. Without browser tabs, a plain explanation. */
export function BrowserTabView({ page, url, available, frozen, slotRef, barRef, onGo, onNav, onOpenOutside, onEscape, announce }: BrowserTabViewProps) {
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
      <div ref={slotRef} className="browser-tab__slot" tabIndex={0} aria-label="Web page">
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

/** A browser tab: the URL bar over a slot, and the page the shell keeps there while the tab is on screen. */
export function BrowserTab({ pageKey, url, active, available = true, onPage, onOpenOutside, onEscape, announce, ref }: BrowserTabProps) {
  const page = usePage(pageKey);
  const slot = useRef<HTMLDivElement>(null);
  // Nothing freezes the page yet: Task 9 turns a cover into a picture here.
  const covered = usePageCovered();
  usePageSlot(slot, pageKey, active && available, covered, url);

  const say = useEffectEvent((u: string, t: string) => onPage(u, t));
  const pageUrl = page?.url ?? "";
  const pageTitle = page?.title ?? "";
  useEffect(() => {
    if (pageUrl !== "") say(pageUrl, pageTitle);
  }, [pageUrl, pageTitle]);

  const go = (to: string) => {
    // A page that is gone opens at the new address; one that exists goes there.
    (page ? browser.nav(pageKey, "go", to) : browser.open(pageKey, to)).catch(ignore);
  };
  return (
    <BrowserTabView
      page={page}
      url={url}
      available={available}
      frozen={null}
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
