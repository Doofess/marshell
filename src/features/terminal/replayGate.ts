// Drops the replies xterm generates while it parses output written before the terminal attached.
// The program asked that query long ago, so a late answer would be typed into whatever it is doing now (#60).

const ESC = "\x1b";
const BEL = "\x07";

/**
 * True when every part of `data` is a reply the terminal sends to a query: OSC and DCS strings, and the CSI
 * replies that `crates/core/src/server/reports.rs` lists. Focus reports, mouse reports, keys and pastes are not.
 */
export function isQueryReply(data: string): boolean {
  if (data.length === 0) return false;
  let at = 0;
  while (at < data.length) {
    const n = replyLength(data, at);
    if (n === null) return false;
    at += n;
  }
  return true;
}

/** The length of the reply that starts at `at`, or null when the text there is anything else (or cut off). */
function replyLength(data: string, at: number): number | null {
  if (data[at] !== ESC) return null;
  switch (data[at + 1]) {
    case "[":
      return csiLength(data, at);
    case "]":
      return stringLength(data, at, true);
    case "P":
      return stringLength(data, at, false);
    default:
      return null;
  }
}

/** OSC and DCS replies run to ST (`ESC \`); an OSC reply may end with BEL instead. */
function stringLength(data: string, at: number, belEnds: boolean): number | null {
  for (let i = at + 2; i < data.length; i++) {
    const c = data[i];
    if (c === BEL && belEnds) return i + 1 - at;
    if (c === ESC) return data[i + 1] === "\\" ? i + 2 - at : null;
  }
  return null;
}

/** Whether the character at `i` exists and lies between `low` and `high`. */
function inRange(data: string, i: number, low: string, high: string): boolean {
  const c = data.charCodeAt(i); // NaN past the end, which compares false
  return c >= low.charCodeAt(0) && c <= high.charCodeAt(0);
}

function csiLength(data: string, at: number): number | null {
  let i = at + 2;
  const paramsStart = i;
  while (inRange(data, i, "0", "?")) i++;
  const prefix = inRange(data, paramsStart, "<", "?") ? data.charAt(paramsStart) : "";
  const intermediatesStart = i;
  while (inRange(data, i, " ", "/")) i++;
  const intermediates = data.slice(intermediatesStart, i);
  if (!inRange(data, i, "@", "~")) return null;
  const last = data.charAt(i);
  const reply =
    (prefix === "" && intermediates === "" && "Rnt".includes(last)) ||
    (prefix === "?" && intermediates === "" && "nRcu".includes(last)) ||
    (prefix === ">" && intermediates === "" && last === "c") ||
    ((prefix === "" || prefix === "?") && intermediates === "$" && last === "y");
  return reply ? i + 1 - at : null;
}

/**
 * The core says where live output starts (`live`), and the terminal says how far it has parsed (`parsed`). While it
 * parses bytes before that offset, a reply it generates answers a query nobody is waiting on, so `allows` is false.
 * Typing, focus and mouse reports always pass.
 */
export class ReplayGate {
  private liveFrom = 0;
  private parsedUpTo = 0;

  /** Output before `seq` was written before this renderer attached. */
  live(seq: number): void {
    this.liveFrom = seq;
  }

  /** The terminal has parsed all output before this byte offset. */
  parsed(upTo: number): void {
    this.parsedUpTo = upTo;
  }

  allows(data: string): boolean {
    return !(this.parsedUpTo < this.liveFrom && isQueryReply(data));
  }
}
