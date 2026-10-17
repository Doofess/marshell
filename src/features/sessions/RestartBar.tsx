import { useEffect, useRef, type KeyboardEvent } from "react";
import { closeBarKey } from "./CloseBar";
import "./CloseBar.css";

/**
 * The bar a Mode change asks with: "Restart 3 billing in Bypass? It ends and runs claude --continue …". The close bar's pattern
 * and look: Cancel takes the focus when it opens (a stray Enter changes nothing), Escape cancels, and it is an alert because it
 * appears in answer to a choice and holds a decision.
 */
export function RestartBar({ text, onCancel, onRestart }: { text: string; onCancel: () => void; onRestart: () => void }) {
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
      <button type="button" className="close-bar__btn" onClick={onRestart}>
        Restart
      </button>
    </div>
  );
}
