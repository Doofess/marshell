import { useRef, type CSSProperties, type KeyboardEvent, type PointerEvent as ReactPointerEvent, type ReactNode } from "react";
import type { TreePosition } from "../../generated/TreePosition";
import "./FilesBody.css";
import { TREE, dividerKey, treeSide, treeWidth } from "./treeLayout";

export type FilesBodyProps = {
  tree: ReactNode;
  filter: ReactNode;
  file: ReactNode;
  pathBar: ReactNode;
  shown: boolean;
  position: TreePosition;
  width: number | null;
  panelWidth: number;
  rtl: boolean;
  overlayOpen: boolean;
  onWidth(w: number | null): void;
  onOverlay(open: boolean): void;
};

/**
 * The Files panel's body: the tree beside the open file with a divider between them, or, under 480 px of panel, over the file.
 * The DOM order is the reading order: the tree first when it is on the start edge (the left in a left-to-right layout, the right in
 * the other), so Tab moves the way the eye reads. The row follows the writing direction as it is.
 */
export function FilesBody({ tree, filter, file, pathBar, shown, position, width, panelWidth, rtl, overlayOpen, onWidth, onOverlay }: FilesBodyProps) {
  const side = treeSide(position, rtl);
  const narrow = panelWidth < TREE.narrow;
  const px = narrow ? Math.min(Math.max(TREE.min, Math.min(width ?? TREE.start, TREE.max)), panelWidth - 40) : treeWidth(width, panelWidth);
  const drag = useRef<{ x: number; width: number } | null>(null);

  const onPointerDown = (e: ReactPointerEvent<HTMLElement>) => {
    if (e.button !== 0) return;
    e.currentTarget.setPointerCapture(e.pointerId);
    drag.current = { x: e.clientX, width: px };
  };
  const onPointerMove = (e: ReactPointerEvent<HTMLElement>) => {
    const s = drag.current;
    if (!s || !e.currentTarget.hasPointerCapture(e.pointerId)) return;
    const next = treeWidth(s.width + (e.clientX - s.x) * (side === "left" ? 1 : -1), panelWidth);
    if (next !== px) onWidth(next);
  };
  const endDrag = () => {
    drag.current = null;
  };
  const onDividerKey = (e: KeyboardEvent<HTMLElement>) => {
    if (e.ctrlKey || e.altKey || e.metaKey) return;
    const next = dividerKey(e.key, px, panelWidth, side);
    if (next === null) return;
    e.preventDefault();
    if (next !== px) onWidth(next);
  };
  const onTreeKey = (e: KeyboardEvent<HTMLElement>) => {
    if (!narrow || e.key !== "Escape") return;
    e.preventDefault();
    e.stopPropagation();
    onOverlay(false);
  };

  const treeColumn = (
    <div key="tree" className="files-body__tree" style={{ inlineSize: `${px}px` } satisfies CSSProperties} onKeyDown={onTreeKey}>
      {filter}
      <div className="files-body__rows">{tree}</div>
    </div>
  );
  const divider = (
    <div
      key="divider"
      className="files-body__divider"
      role="separator"
      aria-orientation="vertical"
      aria-label="Resize the file tree"
      aria-valuenow={px}
      aria-valuemin={TREE.min}
      aria-valuemax={Math.max(TREE.min, panelWidth - TREE.fileMin)}
      tabIndex={0}
      onPointerDown={onPointerDown}
      onPointerMove={onPointerMove}
      onPointerUp={endDrag}
      onPointerCancel={endDrag}
      onLostPointerCapture={endDrag}
      onKeyDown={onDividerKey}
      onDoubleClick={() => onWidth(null)}
    />
  );
  const fileColumn = (
    <div key="file" className="files-body__file">
      {pathBar}
      <div className="files-body__content">{file}</div>
    </div>
  );

  const showTree = shown && (!narrow || overlayOpen);
  // The edge the tree sits on, in the inline direction: the start edge is the left in a left-to-right layout and the right in the other.
  const edge = (side === "left") !== rtl ? "start" : "end";
  // Keys keep the file's frame mounted while the tree comes and goes. In the narrow layout the tree is absolutely placed over the file.
  const parts = showTree ? (narrow ? [fileColumn, treeColumn] : edge === "start" ? [treeColumn, divider, fileColumn] : [fileColumn, divider, treeColumn]) : [fileColumn];
  return (
    <div className="files-body" data-side={side} data-edge={edge} data-narrow={narrow ? "" : undefined} data-rtl={rtl ? "" : undefined}>
      {parts}
    </div>
  );
}
