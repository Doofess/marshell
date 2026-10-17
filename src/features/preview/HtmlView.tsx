import { useMemo, useRef, type SyntheticEvent } from "react";
import { buildSrcdoc } from "./htmlDoc";
import { classifyLink } from "./links";
import type { LinkTarget } from "./links";

type Props = {
  text: string;
  name: string;
  /** The file's path: relative links are resolved against its folder. */
  docPath: string;
  onLink: (link: Exclude<LinkTarget, { kind: "anchor" } | { kind: "none" }>) => void;
  /** A key pressed inside the page, where the pane's own key handling can't see it. Return true if it was used. */
  onKey: (e: KeyboardEvent) => void;
};

/**
 * The HTML file in a sandboxed frame. `sandbox="allow-same-origin"` and nothing else: no script, form, popup or navigation of
 * the window. The same-origin token is there only so this code can reach the frame's document to route its links; the page
 * itself can't run script, and its CSP (see `buildSrcdoc`) lets it load nothing.
 */
export function HtmlView({ text, name, docPath, onLink, onKey }: Props) {
  const srcDoc = useMemo(() => buildSrcdoc(text), [text]);
  // The listeners below live as long as the frame's document; they read the latest props through this.
  const latest = useRef({ docPath, onLink, onKey });
  latest.current = { docPath, onLink, onKey };

  const attach = (e: SyntheticEvent<HTMLIFrameElement>) => {
    const iframe = e.currentTarget;
    const doc = iframe.contentDocument;
    if (!doc) return;
    // Capturing, so nothing in the page sees the click first. Every link is stopped; where it goes is decided here.
    doc.addEventListener(
      "click",
      (ev) => {
        const a = (ev.target as Element | null)?.closest?.("a");
        if (!a) return;
        ev.preventDefault();
        const link = classifyLink(a.getAttribute("href") ?? "", latest.current.docPath);
        if (link.kind === "anchor") {
          const target = doc.getElementById(link.id) ?? doc.getElementsByName(link.id)[0];
          target?.scrollIntoView({ block: "start" });
        } else if (link.kind !== "none") latest.current.onLink(link);
      },
      true,
    );
    // A middle click would open the link in a new window; and a link dragged out would carry its address.
    doc.addEventListener("auxclick", (ev) => ((ev.target as Element | null)?.closest?.("a") ? ev.preventDefault() : undefined), true);
    doc.addEventListener("dragstart", (ev) => ((ev.target as Element | null)?.closest?.("a") ? ev.preventDefault() : undefined), true);
    doc.addEventListener("keydown", (ev) => latest.current.onKey(ev));
    // A page that has just loaded takes the keyboard when the pane's body had it, so the arrow keys scroll it.
    const body = iframe.closest(".preview__body");
    if (body && iframe.ownerDocument.activeElement === body) iframe.focus({ preventScroll: true });
  };

  return <iframe className="html-view" title={`${name}, rendered`} sandbox="allow-same-origin" srcDoc={srcDoc} onLoad={attach} />;
}
