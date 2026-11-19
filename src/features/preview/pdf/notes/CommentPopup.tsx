import { useCallback, useEffect, useLayoutEffect, useRef, useState, type CSSProperties } from "react";
import { useCoverPage } from "../../../browser/pageCover";
import type { CommentRequest } from "./editor";

/** A rectangle in client coordinates. */
export type Box = { left: number; top: number; right: number; bottom: number };

/** How far the popup stands from its note (its width, 280 px at most and 16 px less than the page's, is notes.css's). */
const GAP = 8;

type KeyLike = { key: string; ctrlKey: boolean; metaKey: boolean; altKey: boolean; shiftKey: boolean; isComposing?: boolean };

/** What a key does in the popup: Ctrl+Enter is Done, a plain Esc is Cancel; Enter types a newline, and nothing counts while a composition is open. */
export function popupKey(e: KeyLike): "done" | "cancel" | null {
  if (e.isComposing || e.metaKey || e.altKey || e.shiftKey) return null;
  if (e.key === "Enter" && e.ctrlKey) return "done";
  if (e.key === "Escape" && !e.ctrlKey) return "cancel";
  return null;
}

/**
 * Where the popup goes, relative to its page's box: right of the note with its top at the note's, GAP away; left of it when
 * the right would pass the page's right edge; under it, kept inside the page, when neither side fits. Then kept inside the
 * page vertically.
 */
export function popupPlace(note: Box, page: Box, size: { width: number; height: number }): { left: number; top: number } {
  let left: number;
  let top = note.top;
  if (note.right + GAP + size.width <= page.right) left = note.right + GAP;
  else if (note.left - GAP - size.width >= page.left) left = note.left - GAP - size.width;
  else {
    left = Math.max(page.left, Math.min(note.left, page.right - size.width));
    top = note.bottom + GAP;
  }
  top = Math.max(page.top, Math.min(top, page.bottom - size.height));
  return { left: left - page.left, top: top - page.top };
}

type Props = {
  request: CommentRequest;
  /** The popup is over (it has answered the request, or the request is gone): drop it. */
  onClose: () => void;
  /** Extra style for the popup's box in the page. */
  style?: CSSProperties;
};

/**
 * The comment popup of a note, beside the note inside its page's box. One popup serves one request: draw it with a `key`
 * of its own, so a newer request unmounts it, which cancels it. It takes the keyboard when it opens and gives it back to the
 * note when it goes. It isn't modal (the page stays readable), so Tab goes round inside it, and it ends by Done, Cancel,
 * Delete comment, Esc, Ctrl+Enter, or a pointer down anywhere outside it (a Cancel). Esc and Ctrl+Enter are taken here, in
 * a native listener on its own element, which runs before the pane's and the editor's: neither sees them.
 */
export function CommentPopup({ request, onClose, style }: Props) {
  useCoverPage();
  const root = useRef<HTMLDivElement>(null);
  const field = useRef<HTMLTextAreaElement>(null);
  const ended = useRef(false);
  const mounted = useRef(false);
  const live = useRef({ request, onClose });
  useLayoutEffect(() => {
    live.current = { request, onClose };
  });
  const [at, setAt] = useState<{ left: number; top: number } | null>(null);
  const hasComment = request.text.trim() !== "";

  const end = useCallback((how: "done" | "cancel" | "delete") => {
    if (ended.current) return;
    ended.current = true;
    const { request: req, onClose: close } = live.current;
    let answer: string | null = null;
    if (how === "delete") answer = "";
    else if (how === "done") {
      const text = field.current?.value ?? "";
      // Nothing typed deletes the comment the note had, and changes nothing on a note that had none.
      answer = text.trim() !== "" ? text : req.text.trim() !== "" ? "" : null;
    }
    req.done(answer);
    close();
  }, []);

  // The textarea takes the keyboard, the caret after the text.
  useEffect(() => {
    const ta = field.current;
    if (!ta) return;
    ta.focus({ preventScroll: true });
    ta.setSelectionRange(ta.value.length, ta.value.length);
  }, []);

  // Beside the note, and again whenever the page, the note or the popup changes size (a zoom, a taller textarea).
  useLayoutEffect(() => {
    const el = root.current;
    const page = el?.parentElement;
    if (!el || !page) return;
    const place = () => {
      const next = popupPlace(request.note.getBoundingClientRect(), page.getBoundingClientRect(), { width: el.offsetWidth, height: el.offsetHeight });
      setAt((prev) => (prev && prev.left === next.left && prev.top === next.top ? prev : next));
    };
    place();
    const watch = new ResizeObserver(place);
    watch.observe(page);
    watch.observe(request.note);
    watch.observe(el);
    return () => watch.disconnect();
  }, [request.note]);

  // Esc and Ctrl+Enter end it, and nothing else sees them. Tab goes round inside it.
  useEffect(() => {
    const el = root.current;
    if (!el) return;
    const onKey = (e: KeyboardEvent) => {
      const act = popupKey(e);
      if (act) {
        e.preventDefault();
        e.stopPropagation();
        end(act);
        return;
      }
      if (e.key !== "Tab" || e.ctrlKey || e.altKey || e.metaKey) return;
      const stops = [...el.querySelectorAll<HTMLElement>("textarea, button:not(:disabled)")];
      const first = stops[0];
      const last = stops[stops.length - 1];
      if (!first || !last) return;
      const from = document.activeElement;
      if (e.shiftKey && from === first) {
        e.preventDefault();
        last.focus();
      } else if (!e.shiftKey && from === last) {
        e.preventDefault();
        first.focus();
      }
    };
    const onPointer = (e: PointerEvent) => {
      if (e.target instanceof Node && el.contains(e.target)) return;
      end("cancel");
    };
    el.addEventListener("keydown", onKey);
    document.addEventListener("pointerdown", onPointer, true);
    return () => {
      el.removeEventListener("keydown", onKey);
      document.removeEventListener("pointerdown", onPointer, true);
    };
  }, [end]);

  // Going away, by any road: an answer not yet given is a Cancel (a newer request, or the page dropped), and the keyboard goes
  // back to the note unless the person has already put it somewhere else. Strict mode's trial unmount is told apart by a
  // microtask, which runs only after the trial remount has set `mounted` again.
  useEffect(() => {
    mounted.current = true;
    return () => {
      mounted.current = false;
      queueMicrotask(() => {
        if (mounted.current) return;
        end("cancel");
        const active = document.activeElement;
        if (!active || active === document.body) live.current.request.note.focus({ preventScroll: true });
      });
    };
  }, [end]);

  return (
    <div ref={root} className="pdf-comment" role="dialog" aria-label="Comment" style={{ ...style, insetInlineStart: at?.left, insetBlockStart: at?.top }}>
      <textarea ref={field} className="pdf-comment__field" rows={3} aria-label={`Comment on the note on page ${request.page}`} defaultValue={request.text} />
      <div className="pdf-comment__buttons">
        {hasComment && (
          <button type="button" className="pdf-comment__button pdf-comment__delete" onClick={() => end("delete")}>
            Delete comment
          </button>
        )}
        <button type="button" className="pdf-comment__button" title="Cancel (Esc)" onClick={() => end("cancel")}>
          Cancel
        </button>
        <button type="button" className="pdf-comment__button pdf-comment__done" title="Done (Ctrl+Enter)" onClick={() => end("done")}>
          Done
        </button>
      </div>
    </div>
  );
}
