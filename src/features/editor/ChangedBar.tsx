import { useId } from "react";

type Props = {
  name: string;
  /** Merges the disk's version into the person's text. Absent in a read-only tab, which can't keep the merge (6q-4b decision 7.1). */
  onMerge?: () => void;
  /** Keeps the person's text; the disk's version is what the next save replaces. */
  onKeep: () => void;
  /** Takes the disk's version and drops the person's edits. */
  onTake: () => void;
};

/**
 * The question when the file changed on disk under unsaved edits. It takes no focus when it appears: the person is typing. The
 * announcer says its line, so the bar is not a live region itself.
 */
export function ChangedBar({ name, onMerge, onKeep, onTake }: Props) {
  const line = useId();
  return (
    <div className="changed-bar" role="group" aria-labelledby={line}>
      <p id={line} className="changed-bar__text">{`${name} changed on disk.`}</p>
      <div className="changed-bar__actions">
        {onMerge && (
          <button type="button" className="changed-bar__button" onClick={onMerge}>
            Merge
          </button>
        )}
        <button type="button" className="changed-bar__button" onClick={onKeep}>
          Keep mine
        </button>
        <button type="button" className="changed-bar__button" onClick={onTake}>
          Take disk
        </button>
      </div>
    </div>
  );
}
