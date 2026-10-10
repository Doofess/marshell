/**
 * The Claude logo has twelve rays. These angles were measured from the real outline (degrees clockwise from the
 * positive x axis, y pointing down, the centre at 12,12 of the 24-unit mark): the direction of each ray's tip, and the
 * direction of the narrowest point between two rays. Cutting the mark along the gaps gives twelve wedges, one ray in
 * each, that rebuild the logo exactly and can pulse one after another.
 */
export const CLAUDE_RAY_ANGLES = [351, 12.5, 40.5, 58, 92, 122, 146, 181, 215, 247, 280.5, 312];
export const CLAUDE_VALLEYS = [4.5, 28, 48.5, 74, 101, 128.5, 158, 188, 230, 271, 298, 341];

const CENTRE = 12;
const OUTSIDE = 20;
const n = CLAUDE_VALLEYS.length;

const norm = (deg: number) => ((deg % 360) + 360) % 360;
const point = (deg: number) => {
  const r = (deg * Math.PI) / 180;
  return `${(CENTRE + OUTSIDE * Math.cos(r)).toFixed(3)},${(CENTRE + OUTSIDE * Math.sin(r)).toFixed(3)}`;
};

/** Wedge `i` runs from the gap before ray `i` to the gap after it. */
function span(i: number): [start: number, end: number] {
  return [CLAUDE_VALLEYS[(i + n - 1) % n]!, CLAUDE_VALLEYS[i]!];
}

/** Polygon points for each wedge: the centre, then three points outside the mark along the wedge's arc. */
export const CLAUDE_WEDGES: string[] = CLAUDE_VALLEYS.map((_, i) => {
  const [start, end] = span(i);
  const width = norm(end - start);
  return [`${CENTRE},${CENTRE}`, point(start), point(start + width / 2), point(start + width)].join(" ");
});

/** Which wedge a direction (in degrees) falls in. */
export function wedgeOf(deg: number): number {
  const d = norm(deg);
  for (let i = 0; i < n; i++) {
    const [start, end] = span(i);
    const inside = start <= end ? d >= start && d < end : d >= start || d < end;
    if (inside) return i;
  }
  return -1;
}
