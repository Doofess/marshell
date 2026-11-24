import { type KeyboardEvent as ReactKeyboardEvent, type ReactNode, useEffect, useId, useRef } from "react";
import { takesFocus } from "./CommentBox";
import { threadCopy } from "./threadCopy";

export type ReplyAction = "post" | "close";

/** What a reply may hold, in characters (code points). */
export const MAX_REPLY_CHARS = 65_536;

// Control characters are C0 (0x00-0x1F), DEL and C1 (0x80-0x9F). Not tab and line feed:
const CONTROL_NOT_TAB_LF = /[\u0000-\u0008\u000b-\u001f\u007f-\u009f]/g;

/** What a reply may hold: `\r\n` and `\r` as `\n`, no control character but `\n` and `\t`, at most 65,536 characters (never cut inside a surrogate pair). */
export function cleanReply(text: string): string {
  const clean = text.replace(/\r\n?/g, "\n").replace(CONTROL_NOT_TAB_LF, "");
  if (clean.length <= MAX_REPLY_CHARS) return clean;
  const points = Array.from(clean);
  return points.length <= MAX_REPLY_CHARS ? clean : points.slice(0, MAX_REPLY_CHARS).join("");
}

/** Ctrl+Enter (or Cmd+Enter) posts a reply that isn't blank; Esc closes; Enter is the text area's own (a new line). Null for every other key, and for every key while `posting` or an IME composes. */
export function replyAction(e: { key: string; ctrlKey: boolean; metaKey: boolean; shiftKey: boolean; isComposing?: boolean }, text: string, posting: boolean): ReplyAction | null {
  if (posting || e.isComposing) return null;
  if (e.key === "Escape") return "close";
  if (e.key === "Enter" && (e.ctrlKey || e.metaKey)) return text.trim() === "" ? null : "post";
  return null;
}

export type ReplyBoxProps = {
  label: string;
  value: string;
  /** A reply to this thread is out: the button reads "Replying…" and no key acts. */
  posting: boolean;
  /** The sentence of a refused or unanswered reply, plain text under the text area. */
  error: string | null;
  onChange(text: string): void;
  onAction(a: ReplyAction): void;
  /** Set when the user opened the box; the box takes the focus and clears it. */
  focus: { current: boolean };
};

/** The box under a thread's item to reply on GitHub: a text area, its hint, the button and, after a refusal, the sentence. */
export function ReplyBox({ label, value, posting, error, onChange, onAction, focus }: ReplyBoxProps): ReactNode {
  const text = useRef<HTMLTextAreaElement>(null);
  const hint = useId();
  const sentence = useId();
  useEffect(() => {
    if (!takesFocus(focus)) return;
    const el = text.current;
    el?.focus();
    el?.setSelectionRange(el.value.length, el.value.length);
  }, [focus]);
  const onKeyDown = (e: ReactKeyboardEvent<HTMLElement>) => {
    // The list above takes arrow keys and Enter: none of this reaches it.
    e.stopPropagation();
    const action = replyAction({ key: e.key, ctrlKey: e.ctrlKey, metaKey: e.metaKey, shiftKey: e.shiftKey, isComposing: e.nativeEvent.isComposing }, value, posting);
    if (action === null) return;
    e.preventDefault();
    onAction(action);
  };
  return (
    <div className="changes__box" role="none">
      <textarea ref={text} className="changes__box-text" aria-label={label} aria-describedby={error === null ? hint : `${hint} ${sentence}`} rows={3} value={value} onChange={(e) => onChange(cleanReply(e.target.value))} onKeyDown={onKeyDown} />
      {error !== null && (
        <p className="changes__box-error" id={sentence}>
          {error}
        </p>
      )}
      <div className="changes__box-row">
        <button type="button" className="changes__send-button changes__send-button--primary" aria-disabled={posting ? "true" : undefined} onKeyDown={onKeyDown} onClick={() => !posting && onAction("post")}>
          {posting ? threadCopy.replying : threadCopy.replyButton}
        </button>
        <p className="changes__box-hint" id={hint}>
          {threadCopy.replyHint}
        </p>
      </div>
    </div>
  );
}
