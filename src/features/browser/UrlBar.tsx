import { useImperativeHandle, useRef, useState, type KeyboardEvent, type Ref } from "react";
import { useAnnounceOnce } from "../../lib/useAnnounceOnce";
import { addressFor, BLANK } from "./address";
import type { NavAction } from "./browserApi";
import type { PageState } from "./pageStore";

export type UrlBarHandle = { focusAddress(): void; focusRegion(): void };

export const REFUSED_TEXT = "That address can't be opened in the side pane.";
export const FAILED_TEXT = "This page stopped working. Reload to try again.";
export const POPUP_TEXT = "This page tried to open a tab.";
export const NO_PICTURE_TEXT = "The page's picture couldn't be taken, so there is nothing to select.";
const STATUS_TEXT = { refused: REFUSED_TEXT, failed: FAILED_TEXT, popup: POPUP_TEXT } as const;

export type UrlBarProps = {
  /** The page's state; undefined while it isn't loaded. */
  page: PageState | undefined;
  /** The tab's saved address, shown while the page isn't loaded. */
  fallbackUrl?: string;
  /** Typed text became an address to open or a search. */
  onGo(url: string): void;
  onNav(action: Exclude<NavAction, "go">): void;
  /** Esc in the box: the address is restored, and the owner takes focus away. */
  onEscape(): void;
  /** Starts or ends marking a region on the page. Without it the bar has no "Select region" button. */
  onRegion?: () => void;
  /** The page is live, so a region can be marked on it. */
  canRegion?: boolean;
  /** A region is being marked: the button reads as pressed. */
  selecting?: boolean;
  /** A line under the bar, ahead of the page's own status. */
  note?: string | null;
  /** Says a message through the window's announcer. */
  announce?: (text: string) => void;
  ref?: Ref<UrlBarHandle>;
};

const icon = (d: string) => (
  <svg width="16" height="16" viewBox="0 0 16 16" aria-hidden="true" focusable="false" fill="none" stroke="currentColor" strokeWidth="1.4" strokeLinecap="round" strokeLinejoin="round">
    <path d={d} />
  </svg>
);

const BACK = "M10 3.5 5.5 8l4.5 4.5";
const FORWARD = "M6 3.5 10.5 8 6 12.5";
const RELOAD = "M13 8a5 5 0 1 1-1.6-3.7M13 2.8v2.9h-2.9";
const STOP = "M4 4l8 8M12 4 4 12";
const REGION = "M5 3H3v2M11 3h2v2M13 11v2h-2M5 13H3v-2";

/** Where focus goes after a selection ends: the "Select region" button, or the address box when that button is disabled (the page went blank). */
export function regionFocusTarget(button: { disabled: boolean; focus(): void } | null, box: { focus(): void } | null): { focus(): void } | null {
  return button !== null && !button.disabled ? button : box;
}

/** The browser tab's bar: Back, Forward, Reload or Stop, the address box, and the Select region button. */
export function UrlBar({ page, fallbackUrl = "", onGo, onNav, onEscape, onRegion, canRegion = false, selecting = false, note = null, announce, ref }: UrlBarProps) {
  const input = useRef<HTMLInputElement>(null);
  const regionButton = useRef<HTMLButtonElement>(null);
  // What the person has typed since the box took focus; null shows the page's own address.
  const [draft, setDraft] = useState<string | null>(null);
  const [refused, setRefused] = useState(false);
  useImperativeHandle(ref, () => ({
    focusAddress() {
      input.current?.focus();
      input.current?.select();
    },
    focusRegion() {
      regionFocusTarget(regionButton.current, input.current)?.focus();
    },
  }));

  const address = page?.url || fallbackUrl;
  const loading = page?.loading ?? false;
  const pageStatus = page?.status ? STATUS_TEXT[page.status] : null;
  const status = refused ? REFUSED_TEXT : (note ?? pageStatus);
  useAnnounceOnce(note ?? pageStatus, announce);

  const submit = () => {
    const typed = addressFor(draft ?? (address === BLANK ? "" : address));
    if (typed.kind === "empty") return;
    if (typed.kind === "refused") {
      setRefused(true);
      announce?.(REFUSED_TEXT);
      return;
    }
    setRefused(false);
    setDraft(null);
    onGo(typed.url);
  };
  const onKey = (e: KeyboardEvent<HTMLInputElement>) => {
    if (e.nativeEvent.isComposing) return;
    if (e.key === "Enter") {
      e.preventDefault();
      submit();
    } else if (e.key === "Escape") {
      e.preventDefault();
      setDraft(null);
      setRefused(false);
      onEscape();
    }
  };

  return (
    <>
      <div className="url-bar" role="toolbar" aria-label="Page">
        <button type="button" className="url-bar__button" aria-label="Back" title="Back" disabled={!page?.canBack} onClick={() => onNav("back")}>
          {icon(BACK)}
        </button>
        <button type="button" className="url-bar__button" aria-label="Forward" title="Forward" disabled={!page?.canForward} onClick={() => onNav("forward")}>
          {icon(FORWARD)}
        </button>
        <button
          type="button"
          className="url-bar__button"
          aria-label={loading ? "Stop" : "Reload"}
          title={loading ? "Stop" : "Reload"}
          onClick={() => onNav(loading ? "stop" : "reload")}
        >
          {icon(loading ? STOP : RELOAD)}
        </button>
        <input
          ref={input}
          type="text"
          className="url-bar__box"
          aria-label="Address"
          placeholder="Search Google or type an address"
          spellCheck={false}
          autoComplete="off"
          value={draft ?? (address === BLANK ? "" : address)}
          onChange={(e) => {
            setDraft(e.target.value);
            setRefused(false);
          }}
          onBlur={() => setDraft(null)}
          onKeyDown={onKey}
        />
        {onRegion && (
          <button ref={regionButton} type="button" className="url-bar__button" aria-label="Select region" title="Select region" aria-pressed={selecting} disabled={!canRegion} onClick={onRegion}>
            {icon(REGION)}
          </button>
        )}
      </div>
      {status !== null && <p className="url-bar__status">{status}</p>}
    </>
  );
}
