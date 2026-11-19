import { useId, useLayoutEffect, useMemo, useRef } from "react";

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
 * A ref for the Merge button that notes, as React detaches it and before it leaves the page, whether it had the keyboard. A
 * window blur (the person in another app) fires the button's blur but leaves it the document's active element, so it still
 * counts: the file usually becomes read-only while the person is elsewhere.
 */
export function keyboardOnRemoval<N>(had: { current: boolean }, active: () => unknown): (node: N | null) => (() => void) | undefined {
  return (node) => {
    if (node === null) return undefined;
    return () => {
      had.current = active() === node;
    };
  };
}

/**
 * The question when the file changed on disk under unsaved edits. It takes no focus when it appears: the person is typing. The
 * announcer says its line, so the bar is not a live region itself.
 */
export function ChangedBar({ name, onMerge, onKeep, onTake }: Props) {
  const line = useId();
  const offered = onMerge !== undefined;
  // Merge had the keyboard when it went.
  const mergeFocused = useRef(false);
  const mergeRef = useMemo(() => keyboardOnRemoval<HTMLButtonElement>(mergeFocused, () => document.activeElement), []);
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
          <button ref={mergeRef} type="button" className="changed-bar__button" onClick={onMerge}>
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
