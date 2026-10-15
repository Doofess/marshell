import { SCOPE_LABEL, SCOPE_NOTE, SCOPE_SHAPE, type Scope } from "./scope";
import "./Settings.css";

const SHAPES = {
  circle: <circle cx="6" cy="6" r="4" fill="currentColor" />,
  square: <rect x="2" y="2" width="8" height="8" rx="1" fill="currentColor" />,
  diamond: <rect x="2.5" y="2.5" width="7" height="7" rx="0.5" fill="currentColor" transform="rotate(45 6 6)" />,
  lock: (
    <>
      <rect x="2" y="5.5" width="8" height="5" rx="1" fill="currentColor" />
      <path d="M4 5.5V4a2 2 0 0 1 4 0v1.5" fill="none" stroke="currentColor" strokeWidth="1.3" />
    </>
  ),
  flag: <path d="M3 1.5v9M3 2h6l-1.5 2L9 6H3" fill="currentColor" stroke="currentColor" strokeWidth="1" strokeLinejoin="round" />,
} as const;

/**
 * Where a value comes from: a word, a shape (so it never depends on colour) and, on hover, what it means. The sentence is
 * also in the markup, visually hidden, so a screen reader reads what the word means.
 */
export function ScopeBadge({ scope }: { scope: Scope }) {
  return (
    <span className="scope-badge" data-scope={scope} title={SCOPE_NOTE[scope]}>
      <svg className="scope-badge__shape" width="12" height="12" viewBox="0 0 12 12" aria-hidden="true">
        {SHAPES[SCOPE_SHAPE[scope]]}
      </svg>
      {SCOPE_LABEL[scope]}
      {/* Read after the word by a screen reader, since a tooltip is out of reach of a keyboard. */}
      <span className="scope-badge__note">{SCOPE_NOTE[scope]}</span>
    </span>
  );
}
