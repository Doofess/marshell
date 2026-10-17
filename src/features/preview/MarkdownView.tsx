import { useEffect, useMemo, useRef, type MouseEvent } from "react";
import { HIGHLIGHT_CAP, highlightLines } from "./highlight";
import { classifyLink, type LinkTarget } from "./links";
import { safeMarkdownHtml } from "./markdown";
import { fetchContent } from "./previewApi";
import { imageType } from "./usePreviewFile";

type Props = {
  text: string;
  /** The file's path: relative links and images are resolved against its folder. */
  docPath: string;
  onLink: (link: Exclude<LinkTarget, { kind: "anchor" } | { kind: "none" }>) => void;
};

/** The element a `#id` link points at: headings carry their id as `data-anchor`, so no id of the app's own can clash. */
function anchorTarget(root: HTMLElement, id: string): HTMLElement | null {
  return Array.from(root.querySelectorAll<HTMLElement>("[data-anchor]")).find((e) => e.dataset.anchor === id) ?? null;
}

/** Fenced code, coloured once Shiki has loaded. The colours are spans built with textContent, never markup from a string. */
function colourFences(root: HTMLElement, isStale: () => boolean) {
  for (const code of Array.from(root.querySelectorAll<HTMLElement>("pre > code[class^='language-']"))) {
    const lang = code.className.slice("language-".length);
    const text = code.textContent ?? "";
    if (!lang || text.length > HIGHLIGHT_CAP) continue;
    void highlightLines(text.replace(/\n$/, ""), lang).then((lines) => {
      if (!lines || isStale() || !code.isConnected) return;
      const parts: Node[] = [];
      lines.forEach((line, i) => {
        if (i > 0) parts.push(document.createTextNode("\n"));
        for (const t of line) {
          const span = document.createElement("span");
          span.textContent = t.content;
          if (t.color) span.style.color = t.color;
          if (t.bold) span.style.fontWeight = "600";
          if (t.italic) span.style.fontStyle = "italic";
          if (t.underline) span.style.textDecoration = "underline";
          parts.push(span);
        }
      });
      code.replaceChildren(...parts, document.createTextNode("\n"));
    });
  }
}

/** A local image comes in through the core as a blob; one that can't be read is shown as its alt text. */
function loadImages(root: HTMLElement, urls: string[], isStale: () => boolean) {
  for (const img of Array.from(root.querySelectorAll<HTMLImageElement>("img[data-path]"))) {
    const path = img.dataset.path!;
    const alt = () => img.replaceWith(document.createTextNode(img.alt));
    void fetchContent(path, "image").then(
      (c) => {
        if (isStale()) return;
        if (c.kind !== "ok") return alt();
        const url = URL.createObjectURL(new Blob([c.bytes], { type: imageType(path) }));
        urls.push(url);
        img.addEventListener("error", alt, { once: true });
        img.src = url;
      },
      () => !isStale() && alt(),
    );
  }
}

/**
 * The Markdown file, rendered. The HTML is DOMPurify's output (see `safeMarkdownHtml`), so it is the only thing set as markup.
 * Links never navigate the window: each click is classified and handed to `onLink`, or scrolls to its anchor here.
 */
export function MarkdownView({ text, docPath, onLink }: Props) {
  const html = useMemo(() => safeMarkdownHtml(text, docPath), [text, docPath]);
  const root = useRef<HTMLDivElement>(null);

  // The effect runs after the markup is in; it colours the fences and loads the images, and lets go of the blobs when the markup changes or the view goes.
  useEffect(() => {
    const el = root.current;
    if (!el) return;
    let stale = false;
    const urls: string[] = [];
    colourFences(el, () => stale);
    loadImages(el, urls, () => stale);
    return () => {
      stale = true;
      for (const u of urls) URL.revokeObjectURL(u);
    };
  }, [html]);

  const click = (e: MouseEvent<HTMLElement>) => {
    const a = (e.target as Element).closest?.("a");
    if (!a || !e.currentTarget.contains(a)) return;
    e.preventDefault();
    const link = classifyLink(a.getAttribute("href") ?? "", docPath);
    if (link.kind === "anchor") anchorTarget(e.currentTarget, link.id)?.scrollIntoView({ block: "start" });
    else if (link.kind !== "none") onLink(link);
  };

  return (
    <div
      ref={root}
      className="markdown"
      onClick={click}
      // A middle click would open the link in a new window of the webview; it does nothing here.
      onAuxClick={(e) => (e.target as Element).closest?.("a") && e.preventDefault()}
      dangerouslySetInnerHTML={{ __html: html }}
    />
  );
}
