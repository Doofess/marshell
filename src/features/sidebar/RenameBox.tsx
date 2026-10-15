import { useEffect, useRef, type KeyboardEvent } from "react";
import { renameOutcome } from "./rename";

/**
 * The name of a row while it is being edited (F2): a text box over the row's first line, with the whole name selected.
 * Enter applies, Escape and leaving the box cancel; an empty or unchanged name is refused and the original stays.
 */
export function RenameBox({ original, onCommit, onCancel }: { original: string; onCommit: (name: string) => void; onCancel: (opts: { refocus: boolean }) => void }) {
  const input = useRef<HTMLInputElement>(null);
  // Enter or Escape ends the edit; the blur that follows it (the box going away) must not end it a second time.
  const done = useRef(false);
  useEffect(() => {
    input.current?.focus();
    input.current?.select();
  }, []);
  const onKeyDown = (e: KeyboardEvent<HTMLInputElement>) => {
    const outcome = renameOutcome(e.key, e.currentTarget.value, original);
    if (!outcome) return;
    e.preventDefault();
    done.current = true;
    if (outcome.kind === "commit") onCommit(outcome.name);
    else onCancel({ refocus: true });
  };
  return (
    <input
      ref={input}
      className="session-row__rename"
      type="text"
      defaultValue={original}
      aria-label="Session name"
      dir="auto"
      spellCheck={false}
      autoComplete="off"
      onKeyDown={onKeyDown}
      onBlur={() => {
        if (!done.current) onCancel({ refocus: false });
      }}
    />
  );
}
