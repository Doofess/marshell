import { useMemo, type MouseEvent } from "react";
import { classifyLink, type LinkTarget } from "../preview/links";
import { safeMarkdownHtml } from "../preview/markdown";

/**
 * A path inside the conversation's folder. Relative links resolve against the folder of the "document", so a document at the
 * folder itself would resolve them against its parent.
 */
export const docPathIn = (cwd: string) => `${cwd.replace(/[\\/]+$/, "")}${cwd.includes("\\") ? "\\" : "/"}conversation.md`;

export type MarkdownTextProps = {
  markdown: string;
  /** The conversation's folder: relative links and images are resolved against it. */
  cwd: string;
  /** A file or web link the person clicked. Without it a click does nothing; it never navigates the window. */
  onLink?: (link: Exclude<LinkTarget, { kind: "anchor" } | { kind: "none" }>) => void;
};

/**
 * Assistant text from a transcript. The only markup it sets is DOMPurify's output (`safeMarkdownHtml`, the same path the
 * Preview takes). Without a DOM (the node tests) it is plain text in a pre-wrapped block, so nothing is ever parsed as HTML.
 */
export function MarkdownText({ markdown, cwd, onLink }: MarkdownTextProps) {
  const hasDom = typeof DOMParser !== "undefined";
  const docPath = docPathIn(cwd);
  const html = useMemo(() => (hasDom ? safeMarkdownHtml(markdown, docPath) : null), [hasDom, markdown, docPath]);

  if (html === null) return <div className="history-md history-md--plain">{markdown}</div>;

  const click = (e: MouseEvent<HTMLElement>) => {
    const a = (e.target as Element).closest?.("a");
    if (!a || !e.currentTarget.contains(a)) return;
    e.preventDefault();
    const link = classifyLink(a.getAttribute("href") ?? "", docPath);
    if (link.kind === "anchor") {
      const target = Array.from(e.currentTarget.querySelectorAll<HTMLElement>("[data-anchor]")).find((el) => el.dataset.anchor === link.id);
      target?.scrollIntoView({ block: "nearest" });
    } else if (link.kind !== "none") onLink?.(link);
  };

  return (
    <div
      className="history-md"
      onClick={click}
      // A middle click would open the link in a new window of the webview; it does nothing here.
      onAuxClick={(e) => (e.target as Element).closest?.("a") && e.preventDefault()}
      dangerouslySetInnerHTML={{ __html: html }}
    />
  );
}
