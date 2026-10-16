import "./CommandList.css";

/**
 * The git commands Marshell is about to run, one per line, exactly as the core will run them (docs/PLAN.md: a command is
 * always shown before it runs). Text only: a command is never interpreted as markup.
 */
export function CommandList({ commands }: { commands: string[] }) {
  return (
    <ol className="command-list" aria-label="Commands Marshell will run">
      {commands.map((c, i) => (
        <li key={i} className="command-list__item">
          <code dir="ltr">{c}</code>
        </li>
      ))}
    </ol>
  );
}
