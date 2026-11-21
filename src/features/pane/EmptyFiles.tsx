import { splitPath } from "../preview/kinds";
import { EmptyList, EmptyPanel, FolderIcon, FolderOpenIcon, SearchIcon } from "./EmptyPanel";

/** The Files panel with no tabs: the two ways to open a file, then the project's recent files. */
export function EmptyFiles({ recent, onOpenFile, onBrowse, onRecent }: { recent: readonly string[]; onOpenFile(): void; onBrowse(): void; onRecent(path: string): void }) {
  return (
    <EmptyPanel
      icon={FolderIcon}
      title="No files open"
      line="Open a file from this project, or drop one here."
      actions={[
        { label: "Go to file…", icon: SearchIcon, onClick: onOpenFile },
        { label: "Browse…", icon: FolderOpenIcon, onClick: onBrowse },
      ]}
    >
      {recent.length > 0 && (
        <EmptyList heading="Recent in this project">
          {recent.map((path) => {
            const { name, folder } = splitPath(path);
            return (
              <li key={path}>
                <button type="button" className="empty-panel__row" title={path} onClick={() => onRecent(path)}>
                  <span className="empty-panel__name">{name}</span>
                  {folder && <span className="empty-panel__folder">{folder}</span>}
                </button>
              </li>
            );
          })}
        </EmptyList>
      )}
    </EmptyPanel>
  );
}
