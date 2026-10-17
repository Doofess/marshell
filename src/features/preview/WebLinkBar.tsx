import { useEffect, useId, useRef, type KeyboardEvent } from "react";

type Props = {
  /** The address in full, as `classifyLink` normalised it. It is shown whole: a long one wraps, and is never cut short. */
  url: string;
  onOpen: () => void;
  onCancel: () => void;
};

/** The question before a web link leaves the app. Focus lands on Open, so Enter opens and Escape cancels. */
export function WebLinkBar({ url, onOpen, onCancel }: Props) {
  const open = useRef<HTMLButtonElement>(null);
  const text = useId();
  useEffect(() => {
    open.current?.focus();
  }, [url]);
  const keys = (e: KeyboardEvent<HTMLElement>) => {
    if (e.key !== "Escape" || e.ctrlKey || e.altKey || e.metaKey || e.shiftKey) return;
    // The pane's own Escape would hand focus to the terminal; here it only answers the question.
    e.preventDefault();
    e.stopPropagation();
    onCancel();
  };
  return (
    <div className="web-link" role="group" aria-labelledby={text} onKeyDown={keys}>
      <p id={text} className="web-link__text">
        {"Open "}
        <span className="web-link__url">{url}</span>
        {" in your browser?"}
      </p>
      <div className="web-link__actions">
        <button ref={open} type="button" className="web-link__button" data-primary="true" onClick={onOpen}>
          Open
        </button>
        <button type="button" className="web-link__button" onClick={onCancel}>
          Cancel
        </button>
      </div>
    </div>
  );
}
