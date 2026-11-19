import { useId, useLayoutEffect, useRef } from "react";

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
 * The Merge button went while the keyboard was on it: the file became read-only, and a read-only tab offers no Merge (6q-4b
 * decision 7.1). The browser drops the keyboard to the page, so Keep mine takes it; a keyboard the person moved elsewhere stays.
 */
export function mergeWent(o: { hadFocus: boolean; lost: boolean; keep: { focus(): void } | null }): void {
  if (o.hadFocus && o.lost) o.keep?.focus();
}

/**
 * The question when the file changed on disk under unsaved edits. It takes no focus when it appears: the person is typing. The
 * announcer says its line, so the bar is not a live region itself.
 */
export function ChangedBar({ name, onMerge, onKeep, onTake }: Props) {
  const line = useId();
  const offered = onMerge !== undefined;
  // Merge had the keyboard last. Its removal fires no blur React sees, so this stays set when the button goes with the keyboard.
  const mergeFocused = useRef(false);
  const keep = useRef<HTMLButtonElement>(null);
  useLayoutEffect(() => {
    if (offered) return;
    const active = document.activeElement;
    mergeWent({ hadFocus: mergeFocused.current, lost: active === null || active === document.body, keep: keep.current });
    mergeFocused.current = false;
  }, [offered]);
  return (
    <div className="changed-bar" role="group" aria-labelledby={line}>
      <p id={line} className="changed-bar__text">{`${name} changed on disk.`}</p>
      <div className="changed-bar__actions">
        {onMerge && (
          <button type="button" className="changed-bar__button" onClick={onMerge} onFocus={() => (mergeFocused.current = true)} onBlur={() => (mergeFocused.current = false)}>
            Merge
          </button>
        )}
        <button ref={keep} type="button" className="changed-bar__button" onClick={onKeep}>
          Keep mine
        </button>
        <button type="button" className="changed-bar__button" onClick={onTake}>
          Take disk
        </button>
      </div>
    </div>
  );
}
