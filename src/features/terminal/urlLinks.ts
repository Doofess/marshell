import { openUrl } from "@tauri-apps/plugin-opener";
import type { ILink, ILinkHandler, ILinkProvider } from "@xterm/xterm";
import type { Endpoint } from "../../generated/Endpoint";
import { logicalLine, type BufferLike } from "./pathLinks";

/** An address longer than this is not a link at all. */
const MAX_LENGTH = 2048;
/** From the scheme to whitespace, a control character, `<`, `>`, `"` or a backtick. */
const CANDIDATE = /https?:\/\/[^\s\u0000-\u001f\u007f<>"`]+/gi;
const TRAILING = new Set([".", ",", ";", ":", "!", "?", "'", '"']);
const OPENER = { ")": "(", "]": "[", "}": "{" } as const;

const count = (text: string, ch: string): number => text.split(ch).length - 1;

/** The http and https addresses in a line of output: `start` inclusive, `end` exclusive, in UTF-16 units. */
export function findUrls(text: string): Array<{ start: number; end: number; url: string }> {
  const out: Array<{ start: number; end: number; url: string }> = [];
  for (const m of text.matchAll(CANDIDATE)) {
    let url = m[0];
    for (;;) {
      const last = url.charAt(url.length - 1);
      if (TRAILING.has(last)) {
        url = url.slice(0, -1);
      } else if (last in OPENER && count(url, last) > count(url, OPENER[last as keyof typeof OPENER])) {
        // A closing bracket with no opener inside the address belongs to the sentence around it.
        url = url.slice(0, -1);
      } else {
        break;
      }
    }
    const host = url.slice(url.indexOf("//") + 2);
    if (host.length === 0 || url.length > MAX_LENGTH) continue;
    out.push({ start: m.index, end: m.index + url.length, url });
  }
  return out;
}

/** The tooltip over a clickable web address. */
export const urlHint = (platform: Endpoint["platform"]): string => (platform === "macos" ? "⌘+click to open in your browser" : "Ctrl+click to open in your browser");

type Open = (url: string) => Promise<void>;

/** Hands an http or https address to the system; anything else, or a refusal by the system, does nothing. */
function openWeb(url: string, open: Open): void {
  let protocol: string;
  try {
    protocol = new URL(url).protocol;
  } catch {
    return;
  }
  if (protocol !== "http:" && protocol !== "https:") return;
  try {
    open(url).catch(() => {});
  } catch {
    // A synchronous failure is swallowed too.
  }
}

const modifierHeld = (platform: Endpoint["platform"], event: MouseEvent): boolean => (platform === "macos" ? event.metaKey : event.ctrlKey);

/**
 * Makes the http and https addresses in a terminal's output clickable. Only Ctrl+click (Cmd+click on macOS) opens one, in the
 * system browser: a plain click is left to selection and to a program that reads the mouse.
 */
export function urlLinkProvider(term: { buffer: { active: BufferLike }; element?: HTMLElement | undefined }, platform: Endpoint["platform"], open: Open = openUrl): ILinkProvider {
  return {
    provideLinks(bufferLineNumber, callback) {
      const ll = logicalLine(term.buffer.active, bufferLineNumber - 1);
      const found = findUrls(ll.text).filter((u) => ll.cellOf(u.start).y <= bufferLineNumber && bufferLineNumber <= ll.cellOf(u.end - 1).y);
      if (found.length === 0) return callback(undefined);
      callback(
        found.map((u): ILink => {
          const start = ll.cellOf(u.start);
          const last = ll.cellOf(u.end - 1);
          return {
            range: { start: { x: start.x, y: start.y }, end: { x: last.x + last.width - 1, y: last.y } },
            text: u.url,
            decorations: { underline: true, pointerCursor: true },
            activate(event) {
              if (modifierHeld(platform, event)) openWeb(u.url, open);
            },
            hover: () => term.element?.setAttribute("title", urlHint(platform)),
            leave: () => term.element?.removeAttribute("title"),
          };
        }),
      );
    },
  };
}

/** xterm's handler for OSC 8 hyperlinks, under the same rule as a plain address: the modifier, and only http and https. */
export function webLinkHandler(platform: Endpoint["platform"], element: () => HTMLElement | undefined, open: Open = openUrl): ILinkHandler {
  return {
    activate(event, text) {
      if (modifierHeld(platform, event)) openWeb(text, open);
    },
    hover: () => element()?.setAttribute("title", urlHint(platform)),
    leave: () => element()?.removeAttribute("title"),
  };
}
