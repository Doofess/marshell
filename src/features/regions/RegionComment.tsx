import { useEffect, useId, useRef, useState } from "react";

export type RegionTarget = { name: string; via: "paste" | "clipboard" };

type Props = {
  target: RegionTarget | null;
  busy: boolean;
  error: string | null;
  onAdd: (comment: string) => void;
  /** Esc: back to the box. */
  onCancel: () => void;
};

/** The comment for a marked region: one line, Enter adds it, Esc goes back to the box. */
export function RegionComment({ target, busy, error, onAdd, onCancel }: Props) {
  const [text, setText] = useState("");
  const input = useRef<HTMLInputElement>(null);
  const id = useId();
  useEffect(() => input.current?.focus({ preventScroll: true }), []);
  const disabled = target === null || busy;
  const add = () => {
    if (!disabled) onAdd(text);
  };
  const where = target === null ? "No session to add it to." : target.via === "paste" ? `Adds to ${target.name}` : "Copies to the clipboard";
  return (
    <div className="region-comment" role="group" aria-label="Region comment" onKeyDown={(e) => e.stopPropagation()}>
      <label className="region-comment__label" htmlFor={id}>
        Comment
      </label>
      <input
        ref={input}
        id={id}
        className="region-comment__input"
        type="text"
        autoComplete="off"
        spellCheck
        placeholder="What should the agent look at?"
        value={text}
        disabled={busy}
        onChange={(e) => setText(e.target.value)}
        onKeyDown={(e) => {
          if (e.key === "Enter") {
            e.preventDefault();
            add();
          } else if (e.key === "Escape") {
            e.preventDefault();
            onCancel();
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
