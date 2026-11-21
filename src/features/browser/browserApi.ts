// The app's calls to the shell's pages (src-tauri/src/browser/). Types match the Rust structs in rules.rs field for field.
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { CookieRule } from "../../generated/CookieRule";
import { inTauri } from "../../lib/window/appWindow";

export type Chord = { code: string; ctrl: boolean; shift: boolean; alt: boolean };
export type Rect = { x: number; y: number; width: number; height: number };
export type Size = { width: number; height: number };
/** A straight stretch of the layout rule from window width `from`: x and width as a line over the window's width. */
export type Piece = { from: number; x: number; xPerPx: number; width: number; widthPerPx: number };
/** `bottom` grows by `bottomPerPx` for each pixel the window is taller than `atHeight`, the height `bottom` was measured at. */
export type Rule = { pieces: Piece[]; top: number; bottom: number; bottomPerPx: number; atHeight: number };
/** `rect` is the page's whole place; the shell alone centres and scales a page of `viewport` CSS px in it (rules.rs `fit`). */
export type Report = { rect: Rect; win: Size; rule: Rule | null; moving: boolean; page: string | null; viewport: number | null };

export type NavAction = "go" | "back" | "forward" | "reload" | "stop";
export type FocusReason = "next" | "previous" | "programmatic";

/** A page's key: its session and its pane tab. */
export function pageKey(session: string, paneTab: string): string {
  return `${session}/${paneTab}`;
}

/** Events the shell sends, by name without the `browser://` prefix, with their payloads. */
export type BrowserEvents = {
  state: { key: string; url: string; title: string; canBack: boolean; canForward: boolean; loading: boolean };
  popup: { key: string; url: string };
  /** `why` is "popup" for a tab the page tried to open with no click or key behind it. */
  refused: { key: string; url: string; why?: string };
  failed: { key: string };
  gone: Record<string, never>;
  focus: { key: string; focused: boolean };
  "focus-out": { key: string; direction: "next" | "previous" };
  key: { key: string; code: string; ctrl: boolean; shift: boolean; alt: boolean; repeat: boolean };
  icon: { key: string; src: string | null };
  drag: { key: string; kind: "enter" | "drop" | "leave"; paths?: string[] };
};

// Every call goes through one queue, so React's StrictMode double effects reach the shell in the order they were asked.
// The queue starts closed: its first `tail` is a gate that the first `cookies` call's end (good or bad) opens, so a page is
// never made before the shell knows the rule, and every call asked before it keeps its order behind it. Outside Tauri nothing waits.
let openGate: () => void = () => {};
let told = false;
let tail: Promise<unknown> = inTauri()
  ? new Promise<void>((resolve) => {
      openGate = resolve;
    })
  : Promise.resolve();

function call<T>(command: string, args: Record<string, unknown>, outside: T): Promise<T> {
  if (!inTauri()) return Promise.resolve(outside);
  const result = tail.then(() => invoke<T>(command, args));
  tail = result.catch(() => {});
  return result;
}

/** Tells the shell the rule. The first call goes straight to the shell and its end opens the queue; later ones take their turn in it. */
function cookies(rule: CookieRule): Promise<void> {
  if (!inTauri()) return Promise.resolve();
  if (told) return call<void>("browser_cookies", { rule }, undefined);
  told = true;
  const result = invoke<void>("browser_cookies", { rule });
  result.then(openGate, openGate);
  return result;
}

export const browser = {
  cookies,
  open: (key: string, url: string) => call<void>("browser_open", { key, url }, undefined),
  show: (key: string | null) => call<void>("browser_show", { key }, undefined),
  place: (report: Report) => call<void>("browser_place", { report }, undefined),
  nav: (key: string, action: NavAction, url?: string) => call<void>("browser_nav", { key, action, url: url ?? null }, undefined),
  close: (key: string) => call<void>("browser_close", { key }, undefined),
  keys: (chords: Chord[]) => call<void>("browser_keys", { chords }, undefined),
  focus: (key: string, reason: FocusReason) => call<void>("browser_focus", { key, reason }, undefined),
  /** The page as a `data:image/png;base64,` URL; an empty string outside Tauri. */
  capture: (key: string) => call<string>("browser_capture", { key }, ""),
  /** Waits for the calls asked so far (so the page's show has run), then asks outside the queue: a capture can take seconds, and a show, a place or a close must not wait behind it. */
  screenshot: async (key: string, full: boolean): Promise<{ png: string; cut: boolean }> => {
    if (!inTauri()) return { png: "", cut: false };
    await tail;
    return invoke<{ png: string; cut: boolean }>("browser_screenshot", { key, full });
  },
  theme: (dark: boolean) => call<void>("browser_theme", { dark }, undefined),
  clearData: () => call<void>("browser_clear_data", {}, undefined),
};

/** Listens to one `browser://` event. Returns the stop function, which works even before `listen` has answered. */
export function onBrowser<E extends keyof BrowserEvents>(event: E, handler: (payload: BrowserEvents[E]) => void): () => void {
  if (!inTauri()) return () => {};
  let stopped = false;
  let unlisten: (() => void) | null = null;
  listen<BrowserEvents[E]>(`browser://${event}`, (e) => handler(e.payload))
    .then((fn) => {
      if (stopped) fn();
      else unlisten = fn;
    })
    .catch((error: unknown) => console.error(`Marshell: couldn't listen to browser://${event}.`, error));
  return () => {
    stopped = true;
    unlisten?.();
    unlisten = null;
  };
}
