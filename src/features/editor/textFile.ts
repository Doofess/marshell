export type Encoding = "utf-8" | "utf-8-bom" | "utf-16le" | "utf-16be";
export type Eol = "lf" | "crlf";
export type Decoded = { kind: "text"; text: string; encoding: Encoding; eol: Eol } | { kind: "binary" };

const SNIFF_BYTES = 8192;

/** True when a UTF-16 string has a surrogate with no partner. */
function hasLoneSurrogate(s: string): boolean {
  for (let i = 0; i < s.length; i++) {
    const u = s.charCodeAt(i);
    if (u >= 0xd800 && u <= 0xdbff) {
      const n = s.charCodeAt(i + 1);
      if (!(n >= 0xdc00 && n <= 0xdfff)) return true;
      i++;
    } else if (u >= 0xdc00 && u <= 0xdfff) {
      return true;
    }
  }
  return false;
}

/** The ending the text has more of: `crlf` when `\r\n` beats a lone `\n`, else `lf`. */
function eolOf(text: string): Eol {
  let crlf = 0;
  let lf = 0;
  for (let i = 0; i < text.length; i++) {
    if (text.charCodeAt(i) !== 10) continue;
    if (i > 0 && text.charCodeAt(i - 1) === 13) crlf++;
    else lf++;
  }
  return crlf > lf ? "crlf" : "lf";
}

/**
 * A file's bytes as text. A BOM picks the encoding and is not part of the text; without one the bytes are UTF-8. The text keeps
 * the file's own line endings. A file that could not be written back as it was read is binary: a NUL in the first 8 KB of a UTF-8
 * file, invalid UTF-8, UTF-16 with an odd byte count or with a lone surrogate.
 */
export function decodeFile(bytes: ArrayBuffer): Decoded {
  const all = new Uint8Array(bytes);
  let encoding: Encoding = "utf-8";
  let skip = 0;
  if (all[0] === 0xef && all[1] === 0xbb && all[2] === 0xbf) {
    encoding = "utf-8-bom";
    skip = 3;
  } else if (all[0] === 0xff && all[1] === 0xfe) {
    encoding = "utf-16le";
    skip = 2;
  } else if (all[0] === 0xfe && all[1] === 0xff) {
    encoding = "utf-16be";
    skip = 2;
  }
  const body = all.subarray(skip);
  let text: string;
  try {
    if (encoding === "utf-16le" || encoding === "utf-16be") {
      if (body.length % 2 !== 0) return { kind: "binary" };
      text = new TextDecoder(encoding === "utf-16le" ? "utf-16le" : "utf-16be", { fatal: true, ignoreBOM: true }).decode(body);
      if (hasLoneSurrogate(text)) return { kind: "binary" };
    } else {
      if (body.subarray(0, SNIFF_BYTES).includes(0)) return { kind: "binary" };
      text = new TextDecoder("utf-8", { fatal: true, ignoreBOM: true }).decode(body);
    }
  } catch {
    return { kind: "binary" };
  }
  return { kind: "text", text, encoding, eol: eolOf(text) };
}

/** The bytes to write: every `\r\n`, lone `\r` and lone `\n` in the text becomes the `eol`, then the encoding and its BOM apply. */
export function encodeFile(text: string, encoding: Encoding, eol: Eol): Uint8Array {
  const t = text.replace(/\r\n|\r|\n/g, eol === "crlf" ? "\r\n" : "\n");
  if (encoding === "utf-8" || encoding === "utf-8-bom") {
    const body = new TextEncoder().encode(t);
    if (encoding === "utf-8") return body;
    const out = new Uint8Array(body.length + 3);
    out.set([0xef, 0xbb, 0xbf]);
    out.set(body, 3);
    return out;
  }
  const le = encoding === "utf-16le";
  const out = new Uint8Array(2 + t.length * 2);
  out.set(le ? [0xff, 0xfe] : [0xfe, 0xff]);
  for (let i = 0; i < t.length; i++) {
    const u = t.charCodeAt(i);
    out[2 + i * 2] = le ? u & 0xff : u >> 8;
    out[3 + i * 2] = le ? u >> 8 : u & 0xff;
  }
  return out;
}

const ENCODING_LABELS: Record<Encoding, string> = { "utf-8": "UTF-8", "utf-8-bom": "UTF-8 with BOM", "utf-16le": "UTF-16 LE", "utf-16be": "UTF-16 BE" };

export function encodingLabel(e: Encoding): string {
  return ENCODING_LABELS[e];
}

export function eolLabel(e: Eol): string {
  return e === "crlf" ? "CRLF" : "LF";
}
