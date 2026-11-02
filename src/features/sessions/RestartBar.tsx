import { useEffect, useId, useRef, useState, type KeyboardEvent } from "react";
import { BYPASS_ACK_TEXT } from "../../lib/adminCopy";
import { closeBarKey } from "./CloseBar";
import "./CloseBar.css";

/** Restart is allowed when nothing needs confirming, or the confirmation is ticked. */
export const restartAllowed = (needsAck: boolean, ticked: boolean): boolean => !needsAck || ticked;

/**
 * The bar a Mode change asks with: "Restart 3 billing in Bypass? It ends and runs claude --continue …". The close bar's pattern
 * and look: Cancel takes the focus for a one-line bar (a stray Enter changes nothing), Escape cancels, and it is an alert because it
 * appears in answer to a choice and holds a decision. For an agent running as administrator going to Bypass (`needsAck`) it also
 * holds a checkbox, focused when it opens and toggled by Space, and Restart does nothing until it is ticked.
 */
export function RestartBar({ text, needsAck = false, actionLabel = "Restart", onCancel, onRestart }: { text: string; needsAck?: boolean; actionLabel?: string; onCancel: () => void; onRestart: (acknowledged: boolean) => void }) {
  const root = useRef<HTMLDivElement>(null);
  const ackId = useId();
  const [ticked, setTicked] = useState(false);
  useEffect(() => {
    root.current?.querySelector<HTMLElement>("[data-initial-focus]")?.focus();
  }, []);
  const onKeyDown = (e: KeyboardEvent) => {
    if (closeBarKey(e.key) !== "cancel") return;
    e.preventDefault();
    e.stopPropagation();
    onCancel();
  };
  const allowed = restartAllowed(needsAck, ticked);
  return (
    <div ref={root} className={needsAck ? "close-bar close-bar--ack" : "close-bar"} role="alert" onKeyDown={onKeyDown}>
      <span className="close-bar__text" dir="auto">
        {text}
      </span>
      {needsAck && (
        <label className="close-bar__ack" htmlFor={ackId}>
          <input id={ackId} type="checkbox" data-initial-focus checked={ticked} onChange={(e) => setTicked(e.target.checked)} />
          <span>{BYPASS_ACK_TEXT}</span>
        </label>
      )}
      <button type="button" className="close-bar__btn" data-primary data-initial-focus={!needsAck || undefined} onClick={onCancel}>
        Cancel
      </button>
      <button
        type="button"
        className="close-bar__btn"
        aria-disabled={allowed ? undefined : true}
        onClick={() => {
          if (allowed) onRestart(ticked);
        }}
      >
        {actionLabel}
      </button>
    </div>
  );
}
