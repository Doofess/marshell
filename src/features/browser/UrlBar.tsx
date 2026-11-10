import { useImperativeHandle, useRef, useState, type KeyboardEvent, type Ref } from "react";
import { useAnnounceOnce } from "../../lib/useAnnounceOnce";
import { addressFor, BLANK } from "./address";
import type { NavAction } from "./browserApi";
import type { PageState } from "./pageStore";

export type UrlBarHandle = { focusAddress(): void };

export const REFUSED_TEXT = "That address can't be opened in the side pane.";
export const FAILED_TEXT = "This page stopped working. Reload to try again.";

export type UrlBarProps = {
  /** The page's state; undefined while it isn't loaded. */
  page: PageState | undefined;
  /** The tab's saved address, shown while the page isn't loaded. */
  fallbackUrl?: string;
  /** Typed text became an address to open or a search. */
  onGo(url: string): void;
  onNav(action: Exclude<NavAction, "go">): void;
  /** Open this address in the system browser. */
  onOpenOutside(url: string): void;
  /** Esc in the box: the address is restored, and the owner takes focus away. */
  onEscape(): void;
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
const OUTSIDE = "M9.5 3H13v3.5M13 3 7.5 8.5M11 9.5V11a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V7a2 2 0 0 1 2-2h1.5";

/** The browser tab's bar: Back, Forward, Reload or Stop, the address box, and a button that opens the page in the system browser. */
export function UrlBar({ page, fallbackUrl = "", onGo, onNav, onOpenOutside, onEscape, announce, ref }: UrlBarProps) {
  const input = useRef<HTMLInputElement>(null);
  // What the person has typed since the box took focus; null shows the page's own address.
  const [draft, setDraft] = useState<string | null>(null);
  const [refused, setRefused] = useState(false);
  useImperativeHandle(ref, () => ({
    focusAddress() {
      input.current?.focus();
      input.current?.select();
    },
  }));

  const address = page?.url || fallbackUrl;
  const loading = page?.loading ?? false;
  const pageStatus = page?.status === "refused" ? REFUSED_TEXT : page?.status === "failed" ? FAILED_TEXT : null;
  const status = refused ? REFUSED_TEXT : pageStatus;
  useAnnounceOnce(pageStatus, announce);

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
        <button
          type="button"
          className="url-bar__button"
          aria-label="Open in the system browser"
          title="Open in the system browser"
          disabled={address === "" || address === BLANK}
          onClick={() => onOpenOutside(address)}
        >
          {icon(OUTSIDE)}
        </button>
      </div>
      {status !== null && <p className="url-bar__status">{status}</p>}
    </>
  );
}
