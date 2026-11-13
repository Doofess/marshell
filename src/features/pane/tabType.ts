import { kindOf } from "../preview/kinds";

/** What a side-pane tab holds, as the strip's icon shows it: a web page, a text file of any kind, a PDF or an image. */
export type TabKind = "web" | "text" | "pdf" | "image";

/** A browser tab is `web`; a file is `pdf` or `image` by its kind, and `text` for every other one (code, markdown, html, text). */
export function tabKind(browser: boolean, path: string): TabKind {
  if (browser) return "web";
  const kind = kindOf(path);
  return kind === "pdf" || kind === "image" ? kind : "text";
}

/** A file name cut before its extension (1 to 8 letters or digits after the last dot, which is not the first character), so the tail can keep showing. */
export function splitName(name: string): { head: string; tail: string } {
  const m = /^(.+)(\.[A-Za-z0-9]{1,8})$/.exec(name);
  return m ? { head: m[1]!, tail: m[2]! } : { head: name, tail: "" };
}
