import { useEffect, useRef, type KeyboardEvent } from "react";
import "./CloseBar.css";

/** Escape cancels. Every other key is left alone, so Tab still walks between the two buttons. */
export const closeBarKey = (key: string): "cancel" | null => (key === "Escape" ? "cancel" : null);

/**
 * The header bar a running agent session shows before it closes: "Claude Code may be mid-task in my-app." with Cancel and
 * Close now. Cancel takes the focus when the bar opens, so a stray Enter keeps the session, and Escape cancels. It is an
 * alert because it appears in answer to a key press and holds a decision (the one live region allowed besides the window's
 * announcer: it exists only while the question is open).
 */
export function CloseBar({ text, onCancel, onCloseNow }: { text: string; onCancel: () => void; onCloseNow: () => void }) {
  const root = useRef<HTMLDivElement>(null);
  useEffect(() => {
    root.current?.querySelector<HTMLElement>("[data-initial-focus]")?.focus();
  }, []);
  const onKeyDown = (e: KeyboardEvent) => {
    if (closeBarKey(e.key) !== "cancel") return;
    e.preventDefault();
    e.stopPropagation();
    onCancel();
  };
  return (
    <div ref={root} className="close-bar" role="alert" onKeyDown={onKeyDown}>
      <span className="close-bar__text" dir="auto">
        {text}
      </span>
      <button type="button" className="close-bar__btn" data-primary data-initial-focus onClick={onCancel}>
        Cancel
      </button>
      <button type="button" className="close-bar__btn" onClick={onCloseNow}>
        Close now
      </button>
    </div>
  );
}
