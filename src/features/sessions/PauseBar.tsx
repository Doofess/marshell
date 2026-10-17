import { useEffect, useRef, type KeyboardEvent } from "react";
import { closeBarKey } from "./CloseBar";
import "./CloseBar.css";

/**
 * The two Pause bars, in the close bar's pattern and look: the first-time question ("Pause 3 billing? It freezes the program …",
 * Cancel and Pause) and the one shown when someone types into a paused session ("Paused. Continue to type.", Dismiss and
 * Continue). The first button takes the focus when it opens, so a stray Enter changes nothing; Escape cancels.
 */
export function PauseBar({ text, action, cancel = "Cancel", onCancel, onAction }: { text: string; action: string; cancel?: string; onCancel: () => void; onAction: () => void }) {
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
        {cancel}
      </button>
      <button type="button" className="close-bar__btn" onClick={onAction}>
        {action}
      </button>
    </div>
  );
}
