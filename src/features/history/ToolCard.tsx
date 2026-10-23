import type { ToolCard as Card } from "../../generated/ToolCard";

/**
 * A tool call in one line. With a detail it opens (Enter or Space on the summary) to show the full text, as text in a `<pre>`;
 * without one it is a plain line, not a disclosure with nothing inside. A failed call carries `data-error`, which draws a bar
 * on its start edge (and an outline in forced colours); its summary already says "failed".
 */
export function ToolCard({ card }: { card: Card }) {
  const error = card.error ? "true" : undefined;
  if (!card.detail)
    return (
      <div className="tool tool--plain" data-error={error}>
        <span className="tool__summary">{card.summary}</span>
      </div>
    );
  return (
    <details className="tool" data-error={error}>
      <summary className="tool__summary">{card.summary}</summary>
      <pre className="tool__detail">{card.detail}</pre>
    </details>
  );
}
