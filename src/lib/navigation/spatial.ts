import { sameTarget, type Dir, type NavLayout, type Rect, type Target } from "./types";

const TOUCH = 2;

const span = (r: Rect, horizontal: boolean) => (horizontal ? { from: r.x, to: r.x + r.w } : { from: r.y, to: r.y + r.h });
const overlap = (a: { from: number; to: number }, b: { from: number; to: number }) => Math.min(a.to, b.to) - Math.max(a.from, b.from);
const centre = (r: Rect, horizontal: boolean) => (horizontal ? r.x + r.w / 2 : r.y + r.h / 2);

/** How far past the origin's edge the candidate lies (0 when it touches), or null when it is not in that direction at all. */
function gapAhead(from: Rect, to: Rect, dir: Dir): number | null {
  const gap = dir === "right" ? to.x - (from.x + from.w) : dir === "left" ? from.x - (to.x + to.w) : dir === "down" ? to.y - (from.y + from.h) : from.y - (to.y + to.h);
  return gap >= -TOUCH ? Math.max(0, gap) : null;
}

/**
 * The target an arrow key moves the cursor to, or null when there is nothing further that way.
 * Things whose extent overlaps the origin across the direction of travel win (straight ahead), nearest first, ties broken by how
 * far off-centre they are. Only when nothing is straight ahead does it reach sideways, and then only for left and right (a sidebar
 * row to a pane at another height) or for the top bar from below; a down arrow at the end of the list does not jump to a pane.
 */
export function moveCursor(from: Rect, origin: Target | null, dir: Dir, layout: NavLayout): Target | null {
  const horizontal = dir === "left" || dir === "right";
  const ahead = layout
    .filter((l) => !sameTarget(l.target, origin))
    .flatMap((l) => {
      const gap = gapAhead(from, l.rect, dir);
      if (gap === null) return [];
      const across = overlap(span(from, !horizontal), span(l.rect, !horizontal));
      const off = Math.abs(centre(from, !horizontal) - centre(l.rect, !horizontal));
      return [{ ...l, gap, across, off }];
    });
  const straight = ahead.filter((c) => c.across > 0).sort((a, b) => a.gap + a.off * 0.25 - (b.gap + b.off * 0.25));
  if (straight[0]) return straight[0].target;
  const reach = ahead.filter((c) => horizontal || c.target.kind === "top").sort((a, b) => a.gap + a.off - (b.gap + b.off));
  return reach[0]?.target ?? null;
}
