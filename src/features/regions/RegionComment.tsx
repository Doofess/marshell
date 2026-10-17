import { useId, useState } from "react";
import type { Delivery } from "./deliver";

export type RegionTarget = Delivery;

type Props = {
  target: RegionTarget | null;
  busy: boolean;
  error: string | null;
  onAdd: (comment: string) => void;
  /** Esc: back to the box. */
  onCancel: () => void;
};

/** Esc anywhere in the popover (the input or either button) goes back to the box. No key reaches the pane or the terminal behind it. */
export function popoverKeyDown(e: { key: string; preventDefault: () => void; stopPropagation: () => void }, onCancel: () => void) {
  e.stopPropagation();
  if (e.key === "Escape") {
    e.preventDefault();
    onCancel();
  }
}

/** The comment for a marked region: one line, Enter adds it, Esc goes back to the box. */
export function RegionComment({ target, busy, error, onAdd, onCancel }: Props) {
  const [text, setText] = useState("");
  const id = useId();
  const disabled = target === null || "refused" in target || busy;
  const add = () => {
    if (!disabled) onAdd(text);
  };
  const where = target === null ? "No session to add it to." : "refused" in target ? target.refused : target.via === "paste" ? `Adds to ${target.name}` : "Copies to the clipboard";
  return (
    <div className="region-comment" role="group" aria-label="Region comment" onKeyDown={(e) => popoverKeyDown(e, onCancel)}>
      <label className="region-comment__label" htmlFor={id}>
        Comment
      </label>
      <input
        id={id}
        className="region-comment__input"
        type="text"
        autoComplete="off"
        spellCheck
        placeholder="What should the agent look at?"
        value={text}
        // Read-only, not disabled: a disabled input drops focus to the page while Add is in flight.
        readOnly={busy}
        onChange={(e) => setText(e.target.value)}
        onKeyDown={(e) => {
          if (e.key === "Enter") {
            e.preventDefault();
            add();
          }
        }}
      />
      <p className="region-comment__target">{where}</p>
      {error && (
        <p className="region-comment__error" role="alert">
          {error}
        </p>
      )}
      <div className="region-comment__actions">
        <button type="button" className="region-comment__btn" onClick={onCancel}>
          Cancel
        </button>
        <button type="button" className="region-comment__btn region-comment__btn--primary" aria-disabled={disabled ? "true" : undefined} onClick={add}>
          Add
        </button>
      </div>
    </div>
  );
}
