import type { ReactNode } from "react";
import "./EmptyPanel.css";

/** One of an empty panel's buttons (0 to 2). A button that can't be used stays reachable and says why. */
export type EmptyAction = { label: string; icon: ReactNode; disabled?: string; onClick(): void };

/** A 14 px line icon beside a button's words, drawn in the button's own colour. */
function small(path: ReactNode) {
  return (
    <svg viewBox="0 0 14 14" width="14" height="14" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
      {path}
    </svg>
  );
}

export const PlusIcon = small(<path d="M7 2.5v9M2.5 7h9" />);
export const FileIcon = small(<path d="M3.5 1.5h4.5l2.5 2.5v8.5h-7zM8 1.5V4h2.5" />);
export const SearchIcon = small(
  <>
    <circle cx="6" cy="6" r="3.75" />
    <path d="m8.75 8.75 3.25 3.25" />
  </>,
);
export const FolderOpenIcon = small(<path d="M1.5 11V2.5h3.75l1.5 1.75h4.75v2M1.5 11l1.75-4.75h9.25L10.75 11z" />);

/** The panel's own mark, 40 px: the shape is tinted with the accent and drawn in the quietest text colour. */
function large(shape: ReactNode, lines: ReactNode, name?: string) {
  return (
    <svg className="empty-panel__icon" data-icon={name} viewBox="0 0 40 40" width="40" height="40" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
      <g className="empty-panel__icon-shape">{shape}</g>
      {lines}
    </svg>
  );
}

export const GlobeIcon = large(<circle cx="20" cy="20" r="14.25" />, <path d="M5.75 20h28.5M20 5.75c-8.5 8-8.5 20.5 0 28.5M20 5.75c8.5 8 8.5 20.5 0 28.5" />);
export const FolderIcon = large(
  <path d="M5 12a2.5 2.5 0 0 1 2.5-2.5h8.25l3.5 4H32.5A2.5 2.5 0 0 1 35 16v13a2.5 2.5 0 0 1-2.5 2.5h-25A2.5 2.5 0 0 1 5 29z" />,
  <path d="M5 18.5h30" />,
);

/** The Changes panel's glyph (`PanelIcon`'s 16 px frame with a plus and a minus), scaled to 40 px. */
export const ChangesIcon = large(<rect x="5" y="5" width="30" height="30" rx="5" />, <path d="M20 11.25v10M15 16.25h10M15 27.5h10" />, "panel-changes");

/**
 * What a panel shows with no tabs: its mark, what is missing, what to do, and its ways in (none, one or two). The first button that can be used
 * takes focus when the panel does (`focusPane`); with no button the title does, so the panel's key still moves the keyboard into it and a
 * screen reader reads what is missing. Whatever follows the buttons (a list of ports or recent files) is `children`.
 */
export function EmptyPanel({ icon, title, line, actions, children }: { icon: ReactNode; title: string; line: string; actions: readonly EmptyAction[]; children?: ReactNode }) {
  return (
    <div className="empty-panel">
      <div className="empty-panel__column">
        {icon}
        <h2 className="empty-panel__title" tabIndex={actions.length === 0 ? -1 : undefined}>
          {title}
        </h2>
        <p className="empty-panel__line">{line}</p>
        {actions.length > 0 && (
        <div className="empty-panel__actions">
          {actions.map((a, i) => (
            <button
              key={a.label}
              type="button"
              className={i === 0 ? "empty-panel__button empty-panel__button--primary" : "empty-panel__button"}
              aria-disabled={a.disabled !== undefined ? "true" : undefined}
              title={a.disabled}
              onClick={a.disabled !== undefined ? undefined : a.onClick}
            >
              {a.icon}
              <span>{a.label}</span>
            </button>
          ))}
        </div>
        )}
        {children}
      </div>
    </div>
  );
}

/** A list under the buttons, with its small heading. */
export function EmptyList({ heading, children }: { heading: string; children: ReactNode }) {
  return (
    <section className="empty-panel__group">
      <h3 className="empty-panel__heading">{heading}</h3>
      <ul className="empty-panel__list">{children}</ul>
    </section>
  );
}
