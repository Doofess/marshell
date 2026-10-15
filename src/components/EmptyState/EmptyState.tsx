import "./EmptyState.css";

/** A plain block, not a card: what is empty, one sentence, and at most one way forward. It never announces itself; the window's announcer does. */
export function EmptyState({ title, body, action }: { title: string; body: string; action?: { label: string } }) {
  return (
    <div className="empty-state">
      <p className="empty-state__title">{title}</p>
      <p className="empty-state__body">{body}</p>
      {action && (
        <button type="button" className="empty-state__action">
          {action.label}
        </button>
      )}
    </div>
  );
}
