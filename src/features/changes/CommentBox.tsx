import { type KeyboardEvent as ReactKeyboardEvent, useEffect, useId, useRef } from "react";
import { cleanComment } from "./comments";

export type BoxAction = "add" | "keep" | "close";

export const BOX_HINT = "Enter adds · Shift+Enter starts a new line · Esc closes";

/** "Comment on line 42" or "Comment on lines 10 to 14". */
export const boxLabel = (start: number, end: number): string => (start === end ? `Comment on line ${start}` : `Comment on lines ${start} to ${end}`);

/** What a change of the text becomes: `cleanComment`, so a pasted escape or form feed never reaches a draft. */
export const cleanChange = cleanComment;

/**
 * What a key does in the box: Enter adds (an empty box only closes), Shift+Enter starts a new line (the browser's own), Esc closes
 * and keeps typed text as the comment. Null leaves the key to the textarea.
 */
export function boxAction(e: { key: string; shiftKey: boolean; isComposing?: boolean }, text: string): BoxAction | "newline" | null {
  if (e.isComposing) return null;
  if (e.key === "Enter") return e.shiftKey ? "newline" : text.trim() === "" ? "close" : "add";
  if (e.key === "Escape") return text.trim() === "" ? "close" : "keep";
  return null;
}

export type CommentBoxProps = {
  label: string;
  value: string;
  onChange(text: string): void;
  onAction(action: BoxAction): void;
};

/** The box under a range's last row: a textarea and its hint. It takes the focus when it opens. */
export function CommentBox({ label, value, onChange, onAction }: CommentBoxProps) {
  const text = useRef<HTMLTextAreaElement>(null);
  const hint = useId();
  useEffect(() => {
    const el = text.current;
    el?.focus();
    el?.setSelectionRange(el.value.length, el.value.length);
  }, []);
  const onKeyDown = (e: ReactKeyboardEvent<HTMLTextAreaElement>) => {
    // The list above takes arrow keys and Enter: none of this reaches it.
    e.stopPropagation();
    const action = boxAction({ key: e.key, shiftKey: e.shiftKey, isComposing: e.nativeEvent.isComposing }, value);
    if (action === null || action === "newline") return;
    e.preventDefault();
    onAction(action);
  };
  return (
    <div className="changes__box" role="none">
      <textarea ref={text} className="changes__box-text" aria-label={label} aria-describedby={hint} rows={3} value={value} onChange={(e) => onChange(cleanChange(e.target.value))} onKeyDown={onKeyDown} />
      <p className="changes__box-hint" id={hint}>
        {BOX_HINT}
      </p>
    </div>
  );
}
