import { languageFor } from "../editor/fileTypes";

export type PreviewKind = "code" | "markdown" | "html" | "image" | "pdf" | "text";

const IMAGE = new Set(["png", "jpg", "jpeg", "gif", "webp", "svg", "bmp", "ico", "avif"]);

/** The part of the last path segment after its last dot, lower-cased; "" for none ("Makefile", ".gitignore"). */
function extensionOf(path: string): string {
  const name = splitPath(path).name;
  const dot = name.lastIndexOf(".");
  return dot > 0 ? name.slice(dot + 1).toLowerCase() : "";
}

/** What the preview shows a file as, from its name or extension alone. An unknown extension is text; the bytes decide later whether it really is. */
export function kindOf(path: string): PreviewKind {
  const ext = extensionOf(path);
  if (ext === "pdf") return "pdf";
  if (IMAGE.has(ext)) return "image";
  if (ext === "md" || ext === "markdown" || ext === "mdx") return "markdown";
  if (ext === "html" || ext === "htm") return "html";
  return languageFor(path) ? "code" : "text";
}

/** The Shiki language of a file, or undefined for plain text. */
export function languageOf(path: string): string | undefined {
  return languageFor(path);
}

/** The file name and its folder, for either kind of separator. The folder is empty for a bare name. */
export function splitPath(path: string): { name: string; folder: string } {
  const cut = Math.max(path.lastIndexOf("/"), path.lastIndexOf("\\"));
  return cut < 0 ? { name: path, folder: "" } : { name: path.slice(cut + 1), folder: path.slice(0, cut) };
}

export type Decoded = { kind: "text"; text: string } | { kind: "binary" };

const SNIFF_BYTES = 8192;

/** Bytes as UTF-8 text, or "binary" when a NUL byte sits in the first 8 KB or the bytes are not valid UTF-8. */
export function decodeText(bytes: ArrayBuffer): Decoded {
  const head = new Uint8Array(bytes, 0, Math.min(SNIFF_BYTES, bytes.byteLength));
  if (head.includes(0)) return { kind: "binary" };
  try {
    return { kind: "text", text: new TextDecoder("utf-8", { fatal: true }).decode(bytes) };
  } catch {
    return { kind: "binary" };
  }
}
