import "./SessionNumber.css";

/**
 * A session's number, drawn wherever the session is named. A plain tabular numeral on a quiet fill (no outline), so it is
 * readable without colour. The accessible name of whatever holds it already says "Session N", so the badge is hidden from
 * screen readers instead of being read twice. `title` carries the hint that teaches the phrase "session 6".
 */
export function SessionNumber({ n, title }: { n: number; title?: string }) {
  return (
    <span className="session-number" aria-hidden="true" title={title}>
      {n}
    </span>
  );
}
