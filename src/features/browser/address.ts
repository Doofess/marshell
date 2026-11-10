export const SEARCH_URL = "https://www.google.com/search?q=";
export const BLANK = "about:blank";
/** The Address box's class. The window reads it to tell this box from the others: the app's shortcuts work from it (`keepsOwnKeys`). */
export const ADDRESS_BOX = "url-bar__box";

/** A page reports `about:blank` while its first address loads: that is not where the page is. A blank page that has loaded is. */
export const passingBlank = (url: string, loading: boolean): boolean => loading && url === BLANK;

/** The longest address a browser tab may hold: the layout file refuses a longer one, and with it every save of the layout. */
const URL_MAX = 4096;

/** What typed text becomes. "open": a web address to load. "search": a Google search for the words. "refused": a path, another scheme, or userinfo. "empty": nothing typed. */
export type Typed = { kind: "open" | "search"; url: string } | { kind: "refused" } | { kind: "empty" };

const REFUSED: Typed = { kind: "refused" };

/** A Windows drive, a UNC or other leading backslash, a POSIX root, a home folder, or a `./` or `../` start. */
const PATH_SHAPE = /^(?:[a-zA-Z]:[\\/]|\\|\/|~|\.{1,2}(?:[\\/]|$))/;
/** A scheme, unless what follows the colon is a port (`localhost:3000`, `host:80/x`). */
const SCHEME = /^([a-z][a-z0-9+.-]*):(?!\d+(?:[/?#\s]|$))(?=\S)/i;
const IPV6 = /^\[[0-9a-f:.]+\](?::\d+)?$/i;
const IPV4 = /^\d{1,3}(?:\.\d{1,3}){3}(?::\d+)?$/;
const HOST_PORT = /^([^\s:[\]@]+)(?::(\d+))?$/;
const DOTTED = /^[^.]+(?:\.[^.]+)+$/;

const search = (words: string): Typed => ({ kind: "search", url: SEARCH_URL + encodeURIComponent(words) });

/** Parses `candidate` as an address to open: refused with userinfo, a search for the text when it doesn't parse. */
function build(candidate: string, typed: string): Typed {
  let u: URL;
  try {
    u = new URL(candidate);
  } catch {
    return search(typed);
  }
  if (u.username !== "" || u.password !== "") return REFUSED;
  return { kind: "open", url: u.href };
}

/**
 * What the text typed in the address box means. A path (it can name a person or a project) and any scheme but http, https and
 * about:blank are refused, never searched; so is an address with userinfo.
 */
export function addressFor(text: string): Typed {
  const t = text.trim();
  if (t === "") return { kind: "empty" };
  // A backslash is a path, whatever else the text says.
  if (PATH_SHAPE.test(t) || t.includes("\\")) return REFUSED;

  const scheme = SCHEME.exec(t);
  if (scheme) {
    const name = scheme[1]!.toLowerCase();
    if (name === "about") return t.toLowerCase() === BLANK ? { kind: "open", url: BLANK } : REFUSED;
    if (name !== "http" && name !== "https") return REFUSED;
    const authority = t.replace(/^[a-z]+:\/{0,2}/i, "").split(/[/?#]/, 1)[0]!;
    return authority.includes("@") ? REFUSED : build(t, t);
  }

  // Text with whitespace in front of the first `/`, `?` or `#` is words, even when a slash comes later in it.
  const head = t.split(/[/?#]/, 1)[0]!;
  if (/\s/.test(head)) return search(t);
  if (head.includes("@")) return REFUSED;

  let proto: "http:" | "https:" | null = null;
  if (IPV6.test(head) || IPV4.test(head)) {
    proto = "http:";
  } else {
    const hp = HOST_PORT.exec(head);
    const host = hp?.[1]?.toLowerCase();
    if (hp && host !== undefined) {
      if (host === "localhost" || host.endsWith(".localhost")) proto = "http:";
      else if (DOTTED.test(host) || hp[2] !== undefined) proto = "https:";
    }
  }
  if (proto) return build(`${proto}//${t}`, t);
  // Words with no whitespace and a slash in them (`src/main.rs`) are a relative path, not something to search for.
  if (/[/]/.test(t) && !/\s/.test(t)) return REFUSED;
  return search(t);
}

/** True for an http or https URL with no userinfo, or about:blank: what a browser tab may hold and what the terminal, a chip or a link may ask for. */
export function isPageUrl(url: string): boolean {
  if (url.length > URL_MAX) return false;
  let u: URL;
  try {
    u = new URL(url);
  } catch {
    return false;
  }
  if (u.protocol === "about:") return u.href === BLANK;
  return (u.protocol === "http:" || u.protocol === "https:") && u.username === "" && u.password === "";
}

/** "example.com", "localhost:3000"; "" for about:blank or a string that is no URL. */
export function hostOf(url: string): string {
  try {
    return new URL(url).host;
  } catch {
    return "";
  }
}

/** The page's title, else its host, else "New tab". */
export function pageTitle(title: string | null | undefined, url: string): string {
  if (title != null && title.trim() !== "") return title;
  return hostOf(url) || "New tab";
}

/** Two addresses name one page: equal once parsed (so a missing trailing slash on a bare host and the host's case don't matter); the fragment counts. */
export function sameUrl(a: string, b: string): boolean {
  if (a === b) return true;
  try {
    return new URL(a).href === new URL(b).href;
  } catch {
    return false;
  }
}
