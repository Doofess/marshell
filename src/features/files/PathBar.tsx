import { Fragment, type KeyboardEvent, type MouseEvent } from "react";
import type { Segment } from "./pathSegments";
import "./FilesBody.css";

export type PathBarProps = {
  segments: Segment[];
  windows: boolean;
  treeShown: boolean;
  onReveal(folder: string): void;
  onCollapseAll(): void;
  onToggleTree(): void;
  onMenu(anchor: { x: number; y: number }, opener: HTMLElement): void;
};

const icon = { width: 16, height: 16, viewBox: "0 0 16 16", fill: "none", stroke: "currentColor", strokeWidth: 1.25, strokeLinecap: "round", strokeLinejoin: "round", "aria-hidden": true } as const;

export function PathBar({ segments, windows, treeShown, onReveal, onCollapseAll, onToggleTree, onMenu }: PathBarProps) {
  const sep = windows ? "\\" : "/";
  const onContextMenu = (e: MouseEvent<HTMLElement>) => {
    e.preventDefault();
    onMenu({ x: e.clientX, y: e.clientY }, e.currentTarget);
  };
  const onKeyDown = (e: KeyboardEvent<HTMLElement>) => {
    if (e.key !== "ContextMenu" && !(e.key === "F10" && e.shiftKey)) return;
    e.preventDefault();
    const r = e.currentTarget.getBoundingClientRect();
    onMenu({ x: r.left, y: r.bottom }, e.currentTarget);
  };
  return (
    <div className="path-bar" onContextMenu={onContextMenu}>
      <nav className="path-bar__path" aria-label="File path">
        {segments.map((s, i) => (
          <Fragment key={i}>
            {i > 0 && (
              <span aria-hidden="true" className="path-bar__sep">
                {sep}
              </span>
            )}
            {s.folder === null ? (
              <span className="path-bar__name" data-last={i === segments.length - 1 ? "" : undefined}>
                {s.label}
              </span>
            ) : (
              <button type="button" className="path-bar__folder" onClick={() => onReveal(s.folder!)} onKeyDown={onKeyDown}>
                {s.label}
              </button>
            )}
          </Fragment>
        ))}
      </nav>
      <button type="button" className="path-bar__button" aria-label="Collapse all folders" title="Collapse all folders" onClick={onCollapseAll} onKeyDown={onKeyDown}>
        <svg {...icon}>
          <path d="M4 6l4-3 4 3M4 10l4 3 4-3" />
        </svg>
      </button>
      <button
        type="button"
        className="path-bar__button"
        aria-label="Show file tree"
        aria-pressed={treeShown}
        title={treeShown ? "Hide file tree" : "Show file tree"}
        onClick={onToggleTree}
        onKeyDown={onKeyDown}
      >
        <svg {...icon}>
          <rect x="2.5" y="3" width="11" height="10" rx="1.5" />
          <path d="M9 3v10" />
        </svg>
      </button>
    </div>
  );
}
