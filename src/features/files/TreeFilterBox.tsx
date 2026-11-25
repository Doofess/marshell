import type { KeyboardEvent as ReactKeyboardEvent, Ref } from "react";
import "./FileTree.css";

/** What a key does in the box: Esc clears text, or in an empty box leaves for the terminal; Down goes to the rows. */
export function filterKey(key: string, value: string): "clear" | "escape" | "down" | null {
  if (key === "Escape") return value !== "" ? "clear" : "escape";
  if (key === "ArrowDown") return "down";
  return null;
}

export function TreeFilter(p: {
  value: string;
  onChange(value: string): void;
  onEscape(): void;
  onDown(): void;
  inputRef?: Ref<HTMLInputElement>;
}) {
  const onKeyDown = (e: ReactKeyboardEvent<HTMLInputElement>) => {
    if (e.defaultPrevented || e.ctrlKey || e.altKey || e.metaKey) return;
    const act = filterKey(e.key, p.value);
    if (!act) return;
    e.preventDefault();
    if (act === "clear") p.onChange("");
    else if (act === "escape") p.onEscape();
    else p.onDown();
  };
  return (
    <div className="file-tree-filter-box">
      <input
        ref={p.inputRef}
        className="file-tree-filter"
        type="text"
        value={p.value}
        aria-label="Filter files"
        placeholder="Filter, or ? to search in files"
        autoComplete="off"
        spellCheck={false}
        onChange={(e) => p.onChange(e.target.value)}
        onKeyDown={onKeyDown}
      />
      <svg className="file-tree-filter__mark" width="14" height="14" viewBox="0 0 14 14" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
        <circle cx="6" cy="6" r="3.75" />
        <path d="m8.75 8.75 3.25 3.25" />
      </svg>
    </div>
  );
}
