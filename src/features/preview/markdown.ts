import DOMPurify from "dompurify";
import MarkdownIt from "markdown-it";
import { classifyLink } from "./links";

/** The id GitHub gives a heading: lower-cased, anything but letters, numbers, marks, spaces, `_` and `-` dropped, spaces to hyphens. */
export function slugOf(text: string): string {
  return text
    .toLowerCase()
    .replace(/[^\p{L}\p{N}\p{M}\s_-]/gu, "")
    .replace(/\s/g, "-");
}

const md = new MarkdownIt({ html: false, linkify: true });

// Heading ids: a repeat gets -1, -2…, counted per render (the environment is new for each one).
md.core.ruler.push("heading_ids", (state) => {
  const seen = ((state.env as { slugs?: Set<string> }).slugs ??= new Set());
  const tokens = state.tokens;
  for (let i = 0; i < tokens.length - 1; i++) {
    const open = tokens[i]!;
    if (open.type !== "heading_open") continue;
    const text = (tokens[i + 1]!.children ?? []).filter((t) => t.type === "text" || t.type === "code_inline").map((t) => t.content).join("");
    const base = slugOf(text);
    if (!base) continue;
    let slug = base;
    for (let n = 1; seen.has(slug); n++) slug = `${base}-${n}`;
    seen.add(slug);
    open.attrSet("id", slug);
  }
});

/** The Markdown as HTML. Raw HTML in the source is escaped, not passed on. This is not yet safe to insert: `safeMarkdownHtml` is. */
export function renderMarkdown(src: string): string {
  return md.render(src, {});
}

const FORBIDDEN_TAGS = ["form", "input", "button", "select", "textarea", "iframe", "object", "embed", "style", "link", "meta", "base", "svg", "math"];

/**
 * The Markdown as HTML that is safe to insert in the app's own document. It needs the DOM (DOMPurify).
 *
 * After DOMPurify, a second pass works on an inert parsed copy, so nothing in it can load while it is being changed:
 * every `<img>` loses its `src`; a local one keeps its resolved path in `data-path` for the view to load through the core, and
 * any other (a web address, a data: URL…) is replaced by its alt text. Heading ids move to `data-anchor`, so they can never
 * collide with an id or a window property of the app.
 */
export function safeMarkdownHtml(src: string, docPath: string): string {
  const clean = DOMPurify.sanitize(renderMarkdown(src), { USE_PROFILES: { html: true }, FORBID_TAGS: FORBIDDEN_TAGS, FORBID_ATTR: ["style", "target", "srcset"] });
  const doc = new DOMParser().parseFromString(clean, "text/html");
  for (const img of Array.from(doc.querySelectorAll("img"))) {
    const link = classifyLink(img.getAttribute("src") ?? "", docPath);
    if (link.kind === "local") {
      img.removeAttribute("src");
      img.setAttribute("data-path", link.path);
    } else {
      img.replaceWith(doc.createTextNode(img.getAttribute("alt") ?? ""));
    }
  }
  for (const el of Array.from(doc.querySelectorAll("[id]"))) {
    el.setAttribute("data-anchor", el.id);
    el.removeAttribute("id");
  }
  return doc.body.innerHTML;
}
