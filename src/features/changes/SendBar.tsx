import { useEffect, useRef } from "react";

export type SendBarProps = {
  count: number;
  /** "clipboard" for a session with no prompt line: the button reads Copy. */
  via: "paste" | "clipboard";
  /** The bar asks whether to discard. */
  asking: boolean;
  /** A send is under way. */
  busy: boolean;
  onSend(): void;
  onAsk(): void;
  onDiscard(): void;
  onKeep(): void;
};

const comments = (n: number) => `${n} ${n === 1 ? "comment" : "comments"}`;

/** "Send 3 comments", or "Copy 3 comments" when the session has no prompt line. */
export const sendLabel = (count: number, via: "paste" | "clipboard"): string => `${via === "clipboard" ? "Copy" : "Send"} ${comments(count)}`;

/** The bar under the list while comments exist. */
export function SendBar({ count, via, asking, busy, onSend, onAsk, onDiscard, onKeep }: SendBarProps) {
  // "Discard all" leaves the page as the question replaces it: the keyboard goes to Keep, the choice that loses nothing.
  const keep = useRef<HTMLButtonElement>(null);
  useEffect(() => {
    if (asking) keep.current?.focus();
  }, [asking]);
  if (asking) {
    return (
      <div className="changes__send" role="group" aria-label="Comments">
        <span className="changes__send-ask">{`Discard ${comments(count)}?`}</span>
        <button type="button" className="changes__send-button" onClick={onDiscard}>
          Discard
        </button>
        <button ref={keep} type="button" className="changes__send-button" onClick={onKeep}>
          Keep
        </button>
      </div>
    );
  }
  return (
    <div className="changes__send" role="group" aria-label="Comments">
      <button type="button" className="changes__send-button changes__send-button--primary" aria-disabled={busy ? "true" : undefined} onClick={onSend}>
        {sendLabel(count, via)}
      </button>
      <button type="button" className="changes__send-button" onClick={onAsk}>
        Discard all
      </button>
    </div>
  );
}
