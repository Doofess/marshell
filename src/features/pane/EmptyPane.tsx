import { splitPath } from "../preview/kinds";
import "./SidePane.css";

/** What the side pane shows when the session has no tabs. The first button takes focus when the pane does. */
export function EmptyPane({ recent, onOpenFile, onBrowse, onRecent }: { recent: string[]; onOpenFile(): void; onBrowse(): void; onRecent(path: string): void }) {
  return (
    <div className="empty-pane">
      <p className="empty-pane__lead">No tabs open in this session.</p>
      <div className="empty-pane__actions">
        <button type="button" className="empty-pane__button" onClick={onOpenFile}>
          Open file…
        </button>
        <button type="button" className="empty-pane__button" onClick={onBrowse}>
          Browse…
        </button>
      </div>
      {recent.length > 0 && (
        <section className="empty-pane__recent">
          <h2 className="empty-pane__heading">Recent in this project</h2>
          <ul className="empty-pane__list">
            {recent.map((path) => {
              const { name, folder } = splitPath(path);
              return (
                <li key={path}>
                  <button type="button" className="empty-pane__file" title={path} onClick={() => onRecent(path)}>
                    <span className="empty-pane__name">{name}</span>
                    {folder && <span className="empty-pane__folder">{folder}</span>}
                  </button>
                </li>
              );
            })}
          </ul>
        </section>
      )}
      <p className="empty-pane__hint">Drop a file here to open it.</p>
    </div>
  );
}
