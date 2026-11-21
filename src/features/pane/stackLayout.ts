import { DEFAULT_WEIGHT, type PanelId, type Weights } from "./panels";

/** The stack's limits: a panel is at least `min` px tall, the divider is `divider` px, a key moves it `step` px. */
export const STACK = { min: 160, divider: 1, step: 16 } as const;
/** The largest weight the core accepts. */
const MAX_WEIGHT = 100_000;

/** A panel's weight: the saved one, or the default for one that is missing or not a positive number. */
export const weightOf = (weights: Weights, panel: PanelId): number => {
  const w = weights[panel];
  return typeof w === "number" && Number.isFinite(w) && w > 0 ? w : DEFAULT_WEIGHT;
};

/**
 * The rule before rounding. Each shown panel gets its share of the room by weight; a share under the minimum is held at the
 * minimum and the rest share what is left, again and again until none is under it. When the room can't hold the minimum for each,
 * the shares are equal. `slope` is how much of one more pixel of `total` each panel takes.
 */
function split(total: number, shown: readonly PanelId[], weights: Weights): { heights: number[]; slope: number[] } {
  const n = shown.length;
  if (n === 0) return { heights: [], slope: [] };
  if (!Number.isFinite(total) || !(total > 0)) return { heights: shown.map(() => 0), slope: shown.map(() => 0) };
  const room = Math.max(0, total - (n - 1) * STACK.divider);
  if (room < n * STACK.min) return { heights: shown.map(() => room / n), slope: shown.map(() => 1 / n) };
  const w = shown.map((p) => weightOf(weights, p));
  const held = shown.map(() => false);
  const heights = shown.map(() => 0);
  const slope = shown.map(() => 0);
  for (;;) {
    const free = w.map((_, i) => i).filter((i) => !held[i]);
    const left = room - (n - free.length) * STACK.min;
    const total = free.reduce((s, i) => s + w[i]!, 0);
    const under = free.filter((i) => (left * w[i]!) / total < STACK.min);
    if (under.length === 0) {
      for (const i of free) {
        heights[i] = (left * w[i]!) / total;
        slope[i] = w[i]! / total;
      }
      break;
    }
    for (const i of under) {
      held[i] = true;
      heights[i] = STACK.min;
    }
  }
  return { heights, slope };
}

/** Whole pixels for each shown panel, summing to `total` less the dividers. Proportional to the weights; a panel under 160 takes 160 and the rest share what is left; when `total` can't hold 160 each, equal. Zero for a `total` that is not a positive number. */
export function stackHeights(total: number, shown: readonly PanelId[], weights: Weights): Weights {
  const { heights } = split(total, shown, weights);
  const out: Weights = {};
  if (shown.length === 0) return out;
  const target = Math.max(0, Math.floor(total) - (shown.length - 1) * STACK.divider);
  const whole = heights.map((h) => Math.floor(h + 1e-9));
  // Whole pixels: round down, and each pixel that is left goes to the panel that lost the most to the rounding, the first on a
  // tie. A divider move saves weights for two whole heights, and this gives those two heights back.
  if (Number.isFinite(total) && total > 0) {
    const lost = heights.map((h, i) => h - whole[i]!);
    const order = whole.map((_, i) => i).sort((a, b) => (Math.abs(lost[b]! - lost[a]!) > 1e-9 ? lost[b]! - lost[a]! : a - b));
    const spare = target - whole.reduce((s, h) => s + h, 0);
    for (let k = 0; k < spare; k++) whole[order[k % order.length]!]! += 1;
    // A total that is not whole leaves less room than the shares took: the first panel gives it back.
    if (spare < 0) whole[0] = whole[0]! + spare;
  }
  shown.forEach((p, i) => {
    out[p] = whole[i]!;
  });
  return out;
}

/** How much of one more pixel of `total` goes to the panels below `panel`: 0 to 1, from the same rule before rounding. */
export function belowPerPx(total: number, shown: readonly PanelId[], weights: Weights, panel: PanelId): number {
  const at = shown.indexOf(panel);
  if (at < 0) return 0;
  const { slope } = split(total, shown, weights);
  return slope.slice(at + 1).reduce((s, v) => s + v, 0);
}

/** The pair's heights after the divider between them moved `by` px down: each at least 160, their sum kept. The same heights when the pair can't hold 2 × 160. */
export function movedPair(upperPx: number, lowerPx: number, by: number): { upper: number; lower: number } {
  const sum = upperPx + lowerPx;
  if (!(sum >= 2 * STACK.min) || !Number.isFinite(by)) return { upper: upperPx, lower: lowerPx };
  const upper = Math.min(sum - STACK.min, Math.max(STACK.min, Math.round(upperPx + by)));
  return { upper, lower: sum - upper };
}

/**
 * The weights that give the pair these heights: the pair's weight sum is kept and split in proportion, each at least 1 (and at
 * most the core's 100000, which scales the sum down when a lopsided split would pass it); other weights untouched. The same
 * weights when a height is not a positive number.
 */
export function pairWeights(weights: Weights, upper: PanelId, lower: PanelId, upperPx: number, lowerPx: number): Weights {
  if (!(Number.isFinite(upperPx) && upperPx > 0 && Number.isFinite(lowerPx) && lowerPx > 0)) return weights;
  const share = upperPx / (upperPx + lowerPx);
  let sum = weightOf(weights, upper) + weightOf(weights, lower);
  sum = Math.max(2, Math.min(Math.round(sum), Math.floor(MAX_WEIGHT / Math.max(share, 1 - share))));
  const up = Math.min(sum - 1, Math.max(1, Math.round(sum * share)));
  return { ...weights, [upper]: up, [lower]: sum - up };
}

/** The discrete state a motion keys on: the panels, joined with "+". */
export function stackKey(panels: readonly PanelId[]): string {
  return panels.join("+");
}
