import DOMPurify from "dompurify";
import MarkdownIt from "markdown-it";
import type { Token } from "markdown-it";
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

// A GitHub comment's Markdown, from a stranger and possibly read by an agent: nothing in its source may go unseen. The rules that
// would swallow text are off (a link reference definition, a table's cells past its columns, a character entity that decodes to
// an invisible character); an autolink's text is drawn as written; a picture is a link to its https address, or else its alt text
// and its address as text; any other link has its address on hover (https) or as text; a title is text, its hidden characters as
// source; and a fence's whole info string is shown.
const commentMd = new MarkdownIt({ html: false, linkify: true });
commentMd.disable(["reference", "table", "entity"]);
commentMd.normalizeLinkText = (text) => text;

/** A title as text a reader sees whole: markdown-it decodes entities in a title even with the entity rule off, so each hidden character (default-ignorable, control, format) is put back as `&#x<HEX>;`, and a line end is a space. */
function titleText(title: string): string {
  return title.replace(/[\p{Default_Ignorable_Code_Point}\p{Cc}\p{Cf}]/gu, (c) => (c === "\n" || c === "\r" ? " " : `&#x${c.codePointAt(0)!.toString(16).toUpperCase()};`));
}

/** The text of inline tokens, as a reader sees it: text, code, and for a link or a picture inside a picture's description its address and title after its words. */
function textOf(tokens: Token[] | null): string {
  let out = "";
  const links: Token[] = [];
  for (const t of tokens ?? []) {
    if (t.type === "image") {
      const src = String(t.attrGet("src") ?? "");
      const title = String(t.attrGet("title") ?? "");
      out += textOf(t.children) + (src === "" ? "" : ` (${src})`) + (title === "" ? "" : ` (${titleText(title)})`);
    } else if (t.type === "link_open") links.push(t);
    else if (t.type === "link_close") {
      const link = links.pop();
      const href = String(link?.attrGet("href") ?? "");
      const title = String(link?.attrGet("title") ?? "");
      out += (href === "" ? "" : ` (${href})`) + (title === "" ? "" : ` (${titleText(title)})`);
    } else if (t.type === "softbreak" || t.type === "hardbreak") out += " ";
    else out += t.content;
  }
  return out;
}

/** Whether a link's inside draws anything: a picture (it always has words), or a letter, number or mark that isn't a blank. */
const draws = (tokens: Token[]): boolean => tokens.some((t) => t.type === "image") || /[\p{L}\p{N}\p{P}\p{S}]/u.test(textOf(tokens).replace(/⠀/g, ""));

/** Whether `href` is an address that parses with the https scheme. */
function httpsAddress(href: string): boolean {
  try {
    return commentMd.validateLink(href) && new URL(href).protocol === "https:";
  } catch {
    return false;
  }
}

/** A link: an https address goes on its tag as the hover title; any other follows the link's text as ` (address)`, unless the text is the address; its own title follows as text and leaves the tag. A link that draws nothing shows its address as its text. */
commentMd.core.ruler.push("unseen_links", (state) => {
  const text = (content: string): Token => Object.assign(new state.Token("text", "", 0), { content });
  for (const block of state.tokens) {
    if (block.type !== "inline" || block.children === null) continue;
    const out: Token[] = [];
    const open: { link: Token; from: number }[] = [];
    for (const t of block.children) {
      if (t.type === "link_open") open.push({ link: t, from: out.length + 1 });
      // A link inside a link isn't HTML: a picture in a link is its words and its address.
      if (t.type === "image" && open.length > 0) t.meta = { ...(t.meta as object | null), inLink: true };
      const last = t.type === "link_close" ? open.pop() : undefined;
      if (last === undefined) {
        out.push(t);
        continue;
      }
      const href = String(last.link.attrGet("href") ?? "");
      if (!draws(out.slice(last.from))) out.push(text(href));
      const words = textOf(out.slice(last.from));
      const title = String(last.link.attrGet("title") ?? "");
      last.link.attrs = (last.link.attrs ?? []).filter(([name]) => name !== "title");
      const https = httpsAddress(href);
      if (https) last.link.attrSet("title", href);
      out.push(t);
      if (!https && words !== href && words !== href.replace(/^mailto:/, "")) out.push(text(` (${href})`));
      if (title !== "") out.push(text(` (${titleText(title)})`));
    }
    block.children = out;
  }
});

/** A picture would load from an address the reader never chose. Alone with an https address it is a link to it (the address on hover); any other picture is its alt text and ` (address)` as text. Its title follows as text. */
commentMd.renderer.rules.image = (tokens, idx) => {
  const token = tokens[idx]!;
  const escape = commentMd.utils.escapeHtml;
  const alt = escape(textOf(token.children).trim() || "Picture");
  const title = String(token.attrGet("title") ?? "");
  const after = title === "" ? "" : ` (${escape(titleText(title))})`;
  const src = String(token.attrGet("src") ?? "");
  if (httpsAddress(src) && !(token.meta as { inLink?: boolean } | null)?.inLink) return `<a href="${escape(src)}" title="${escape(src)}">${alt}</a>${after}`;
  return alt + (src === "" ? "" : ` (${escape(src)})`) + after;
};

const fenceBlock = commentMd.renderer.rules.fence!;
commentMd.renderer.rules.fence = (tokens, idx, options, env, self) => {
  const info = tokens[idx]!.info.trim();
  return (info === "" ? "" : `<p class="markdown__info">${commentMd.utils.escapeHtml(info)}</p>\n`) + fenceBlock(tokens, idx, options, env, self);
};

/**
 * A comment's Markdown as HTML, drawn so that nothing in the source is unseen: as `renderMarkdown`, with no heading ids. A picture alone
 * with an https address is a link to it (its alt text, or "Picture"; the address on hover); any other picture is its alt text and its
 * address as text. A link's https address is on hover, any other address follows its text. A title follows as text, its hidden
 * characters as `&#x…;`; the address and title of a link or picture in a picture's description follow its words. A link reference
 * definition, a character entity and a table show as their source text; a code fence shows its whole info string above the code.
 * Not yet safe to insert.
 */
export function renderCommentMarkdown(src: string): string {
  return commentMd.render(src, {});
}

/** `make`, remembering its last `max` answers by source (the list redraws often, and cleaning HTML is slow). */
export function remembered(make: (src: string) => string, max: number): (src: string) => string {
  const seen = new Map<string, string>();
  return (src) => {
    const known = seen.get(src);
    if (known !== undefined) return known;
    const html = make(src);
    seen.set(src, html);
    if (seen.size > max) seen.delete(seen.keys().next().value!);
    return html;
  };
}

/** The Markdown as HTML. Raw HTML in the source is escaped, not passed on. This is not yet safe to insert: `safeMarkdownHtml` is. */
export function renderMarkdown(src: string): string {
  return md.render(src, {});
}

const FORBIDDEN_TAGS = ["form", "input", "button", "select", "textarea", "iframe", "object", "embed", "style", "link", "meta", "base", "svg", "math"];
/** What DOMPurify keeps of the Markdown's HTML: the app's own pages and a GitHub comment are cleaned alike. */
const SANITIZE = { USE_PROFILES: { html: true }, FORBID_TAGS: FORBIDDEN_TAGS, FORBID_ATTR: ["style", "target", "srcset"] };

/**
 * The Markdown as HTML that is safe to insert in the app's own document. It needs the DOM (DOMPurify).
 *
 * After DOMPurify, a second pass works on an inert parsed copy, so nothing in it can load while it is being changed:
 * every `<img>` loses its `src`; a local one keeps its resolved path in `data-path` for the view to load through the core, and
 * any other (a web address, a data: URL…) is replaced by its alt text. Heading ids move to `data-anchor`, so they can never
 * collide with an id or a window property of the app.
 */
export function safeMarkdownHtml(src: string, docPath: string): string {
  const clean = DOMPurify.sanitize(renderMarkdown(src), SANITIZE);
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

/**
 * `renderCommentMarkdown` through DOMPurify (the options `safeMarkdownHtml` uses), with every link given `tabindex="-1"` (the keyboard
 * stays on the thread's item) and no element keeping an id. Needs the DOM.
 */
export function safeCommentHtml(src: string): string {
  const doc = new DOMParser().parseFromString(DOMPurify.sanitize(renderCommentMarkdown(src), SANITIZE), "text/html");
  for (const a of Array.from(doc.querySelectorAll("a"))) a.setAttribute("tabindex", "-1");
  for (const el of Array.from(doc.querySelectorAll("[id]"))) el.removeAttribute("id");
  return doc.body.innerHTML;
}
