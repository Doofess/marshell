export const SEARCH_URL = "https://www.google.com/search?q=";
export const BLANK = "about:blank";
/** The Address box's class (UrlBar writes it out, as the style tests need; a test holds the two together). The window reads it to tell this box from the others: the app's shortcuts work from it (`keepsOwnKeys`). */
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

/** The names the shell serves from inside every webview (`SHELL_NAMES` in `src-tauri/src/browser/rules.rs`). */
const SHELL_NAMES = ["tauri", "ipc"];

type HostName = { name: string; thisComputer: boolean };

/** The groups of an IPv6 address written without its brackets, or null when it is not one. */
function ipv6Groups(text: string): number[] | null {
  let tail: number[] = [];
  let body = text;
  const v4 = /(?:^|:)(\d{1,3})\.(\d{1,3})\.(\d{1,3})\.(\d{1,3})$/.exec(text);
  if (v4) {
    const [a, b, c, d] = v4.slice(1).map(Number) as [number, number, number, number];
    if ([a, b, c, d].some((n) => n > 255)) return null;
    tail = [a * 256 + b, c * 256 + d];
    body = text.slice(0, text.length - v4[0].length + (v4[0].startsWith(":") ? 1 : 0));
    if (body === ":") body = "::";
  }
  const parts = body.split("::");
  if (parts.length > 2) return null;
  const groups = (list: string): number[] | null => {
    if (list === "") return [];
    const out = list.split(":").map((g) => (/^[0-9a-f]{1,4}$/i.test(g) ? parseInt(g, 16) : NaN));
    return out.some(Number.isNaN) ? null : out;
  };
  const head = groups(parts[0]!);
  const rest = parts.length === 2 ? groups(parts[1]!) : [];
  if (head === null || rest === null) return null;
  if (parts.length === 1) return head.length + tail.length === 8 ? [...head, ...tail] : null;
  const missing = 8 - head.length - rest.length - tail.length;
  return missing < 1 ? null : [...head, ...Array<number>(missing).fill(0), ...rest, ...tail];
}

/** A host as `HostName::parse` reads it in rules.rs: lower case, no trailing dot, and whether it names this computer. */
function hostName(host: string): HostName {
  const name = host.replace(/\.+$/, "").toLowerCase();
  const loopbackOrAny = (octets: readonly number[]) => octets[0] === 127 || octets.every((n) => n === 0);
  const v4 = /^(\d{1,3})\.(\d{1,3})\.(\d{1,3})\.(\d{1,3})$/.exec(name);
  if (v4) {
    const octets = v4.slice(1).map(Number);
    // Rust's Ipv4Addr parser refuses an octet over 255 and falls through to a name.
    if (octets.every((n) => n <= 255)) return { name, thisComputer: loopbackOrAny(octets) };
  }
  if (name.startsWith("[") && name.endsWith("]")) {
    const g = ipv6Groups(name.slice(1, -1));
    if (g) {
      const loopback = g.slice(0, 7).every((n) => n === 0) && g[7] === 1;
      const unspecified = g.every((n) => n === 0);
      const mapped = g.slice(0, 5).every((n) => n === 0) && g[5] === 0xffff && loopbackOrAny([g[6]! >> 8, g[6]! & 255, g[7]! >> 8, g[7]! & 255]);
      return { name, thisComputer: loopback || unspecified || mapped };
    }
  }
  return { name, thisComputer: name === "localhost" || name.endsWith(".localhost") };
}

/** Whether the shell answers this address itself, or Tauri takes it for the app's own origin (`is_the_shells`). */
function isTheShells(host: HostName, scheme: string): boolean {
  const first = host.name.split(".")[0] ?? "";
  if (scheme === "http:" && SHELL_NAMES.includes(first)) return true;
  return SHELL_NAMES.some((shell) => host.name.endsWith(".localhost") && (host.name.slice(0, -".localhost".length) === shell || host.name.slice(0, -".localhost".length).endsWith(`.${shell}`)));
}

/**
 * The mirror of `allowed_navigation` in `src-tauri/src/browser/rules.rs`: true when the shell would refuse this address (a user
 * name or password, the shell's own names, any of the app's own origins in `own`, a scheme but http, https and about:blank). Asked
 * before a tab is made, so an address the shell refuses never gets one. It must never allow what the shell refuses: both read the
 * same cases (`allowed.json`).
 */
export function shellRefuses(url: string, own: readonly (readonly [string, number])[]): boolean {
  let u: URL;
  try {
    u = new URL(url);
  } catch {
    return true;
  }
  if (u.protocol === "about:") return u.href !== BLANK;
  if (u.protocol !== "http:" && u.protocol !== "https:") return true;
  if (u.username !== "" || u.password !== "") return true;
  const host = hostName(u.hostname);
  if (host.name === "" || isTheShells(host, u.protocol)) return true;
  // The URL parser leaves a scheme's own port out.
  const port = u.port === "" ? (u.protocol === "https:" ? 443 : 80) : Number(u.port);
  return own.some(([ownHost, ownPort]) => {
    if (ownPort !== port) return false;
    const mine = hostName(ownHost);
    return (host.thisComputer && mine.thisComputer) || host.name === mine.name;
  });
}

/**
 * The app's own origins, which a page may never open (`own_origins` in src-tauri/src/browser/mod.rs): where the window is loaded from
 * only under `tauri dev` (the dev server; an installed build loads `http://tauri.localhost/`, which the shell refuses anyway), and
 * the local API's port on 127.0.0.1.
 */
export function ownOrigins(location: { protocol: string; hostname: string; port: string }, apiPort: number | null, dev: boolean): [string, number][] {
  const list: [string, number][] = [];
  const web = location.protocol === "http:" || location.protocol === "https:";
  if (dev && web) list.push([location.hostname, location.port !== "" ? Number(location.port) : location.protocol === "https:" ? 443 : 80]);
  if (apiPort !== null) list.push(["127.0.0.1", apiPort]);
  return list;
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
