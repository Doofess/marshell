// The pictures the window-buttons strip shows (docs/PLAN.md §9, window-buttons strip decisions). The native strip draws nothing of
// its own: the page paints each button's two pictures here, from the colours its own elements have now, so the strip can't drift
// from the page's theme, and sends them as RGBA bytes.

/** Geometry in CSS pixels: a button is 46 wide, the bar 40 tall with a 1 px hairline at its foot, a glyph 10 square. */
export const BUTTON_W = 46;
export const BAR_H = 40;
export const GLYPH = 10;

/** The glyph paths, in a 10 x 10 box. The page's buttons draw the same ones, so the two can't differ. */
export const GLYPHS = {
  minimize: "M0 5h10",
  maximize: "M0.5 0.5h9v9h-9z",
  restore: "M2.5 2.5v-2h7v7h-2M0.5 2.5h7v7h-7z",
  close: "M0 0l10 10M10 0L0 10",
} as const;

/** The strip's size in whole device pixels. `edges` are the buttons' left edges and the strip's right edge. */
export type TileLayout = { dpr: number; height: number; hairline: number; edges: [number, number, number, number] };

/**
 * Edges are rounded from the scale, not each width, so the three pictures add up to the strip's width at a fractional scale.
 * Each is measured from the bar's right end and rounded there, because that is where the page's own buttons end: a window is a
 * whole number of device pixels wide, and the page rounds each edge to a device pixel (a half goes right). At 1.25 the buttons
 * are 57, 58 and 57 wide. This is reasoned from how the page lays out; only a scale of 1 was measured (live check, C2).
 */
export function tileLayout(dpr: number): TileLayout {
  const fromEnd = (i: number) => Math.round((i - 3) * BUTTON_W * dpr);
  const edge = (i: number) => fromEnd(i) - fromEnd(0);
  return { dpr, height: Math.round(BAR_H * dpr), hairline: Math.max(1, Math.floor(dpr)), edges: [edge(0), edge(1), edge(2), edge(3)] };
}

/** A button's colours: its fill and its glyph's stroke, as CSS colour strings. */
export type Look = { bg: string; fg: string };
export type StripLooks = { bar: string; hairline: string; rest: Look; hover: Look; closeHover: Look };
export type ButtonPaint = { width: number; rest: number[]; hover: number[] };
export type StripPaint = { height: number; minimize: ButtonPaint; maximize: ButtonPaint; restore: ButtonPaint; close: ButtonPaint };

/** A 2D context with the way to make a path from a glyph, so a test can stand in for a canvas. */
export type Surface = { ctx: CanvasRenderingContext2D; path(d: string): Path2D };

/** A fill no theme uses: if it is still there after a colour is set, the canvas refused that colour (as in windowBackground.ts). */
const UNSET = "#010203";

/** Sets a fill or stroke colour; false when the canvas refused it. */
function setColour(ctx: CanvasRenderingContext2D, key: "fillStyle" | "strokeStyle", css: string): boolean {
  ctx[key] = UNSET;
  ctx[key] = css;
  return ctx[key] !== UNSET;
}

/** A slot's left edge and width in device pixels. */
function span(layout: TileLayout, slot: 0 | 1 | 2): [number, number] {
  const left = layout.edges[slot] ?? 0;
  return [left, (layout.edges[slot + 1] ?? left) - left];
}

/** One button's picture: the bar, the hairline, the button's fill, then the glyph, read back as RGBA. Null when a colour is refused. */
function paintOne(layout: TileLayout, slot: 0 | 1 | 2, glyph: string, bar: string, hairline: string, look: Look, make: (width: number, height: number) => Surface | null): number[] | null {
  const { dpr, height, hairline: rule } = layout;
  const [left, width] = span(layout, slot);
  const surface = make(width, height);
  if (!surface) return null;
  const { ctx } = surface;
  if (!setColour(ctx, "fillStyle", bar)) return null;
  ctx.fillRect(0, 0, width, height);
  if (!setColour(ctx, "fillStyle", hairline)) return null;
  ctx.fillRect(0, height - rule, width, rule);
  if (!setColour(ctx, "fillStyle", look.bg)) return null;
  ctx.fillRect(0, 0, width, height - rule);
  if (!setColour(ctx, "strokeStyle", look.fg)) return null;
  // The glyph is centred in the button, which fills the rows above the hairline, as the page's own button centres its icon.
  // The page then gives the icon a box of whole device pixels, each edge rounded on its own (14.5 down becomes 15 at a scale
  // of 1; a 12.5-pixel icon becomes 12 or 13), and fits the drawing to that box. Across is measured from the bar's right end,
  // as the edges are, and down within the rows above the hairline. Measured at a scale of 1 only (live check, C2).
  const size = GLYPH * dpr;
  const across = ((slot - 3) * BUTTON_W + (BUTTON_W - GLYPH) / 2) * dpr;
  const down = (height - rule - size) / 2;
  const x = Math.round(across) + (layout.edges[3] ?? 0) - left;
  const y = Math.round(down);
  const w = Math.round(across + size) - Math.round(across);
  const h = Math.round(down + size) - y;
  // The page cuts an icon to its own box, which takes the ends off Close's two strokes.
  ctx.beginPath();
  ctx.rect(x, y, w, h);
  ctx.clip();
  ctx.setTransform(w / GLYPH, 0, 0, h / GLYPH, x, y);
  ctx.lineWidth = 1;
  ctx.stroke(surface.path(glyph));
  return Array.from(ctx.getImageData(0, 0, width, height).data);
}

/** All four buttons' pictures, or null when a colour is refused or no canvas can be made (the strip then stays hidden). */
export function paintStrip(layout: TileLayout, looks: StripLooks, make: (width: number, height: number) => Surface | null): StripPaint | null {
  const button = (slot: 0 | 1 | 2, glyph: string, hover: Look): ButtonPaint | null => {
    const rest = paintOne(layout, slot, glyph, looks.bar, looks.hairline, looks.rest, make);
    const lit = paintOne(layout, slot, glyph, looks.bar, looks.hairline, hover, make);
    return rest && lit ? { width: span(layout, slot)[1], rest, hover: lit } : null;
  };
  const minimize = button(0, GLYPHS.minimize, looks.hover);
  const maximize = button(1, GLYPHS.maximize, looks.hover);
  const restore = button(1, GLYPHS.restore, looks.hover);
  const close = button(2, GLYPHS.close, looks.closeHover);
  return minimize && maximize && restore && close ? { height: layout.height, minimize, maximize, restore, close } : null;
}

/**
 * The looks as the page's own elements have them now, or null when an element is missing. Computed styles give the tokens' `oklch()`
 * as written, which a canvas understands; in forced colours they already hold the system's colours. Every look comes from a
 * hidden probe button, found by its `data-look` attribute: the two hover probes take the hover rules by it, so the pointer need
 * not be on a button, and the rest probe is never under the pointer, as the page's own buttons are whenever the strip is hidden.
 */
export function readLooks(controls: HTMLElement): StripLooks | null {
  const view = controls.ownerDocument.defaultView;
  const bar = controls.closest<HTMLElement>(".window__bar");
  const probe = (selector: string) => controls.parentElement?.querySelector<HTMLElement>(selector);
  const rest = probe('.window__control[data-look="rest"]');
  const hover = probe('.window__control[data-look="hover"]:not(.window__control--close)');
  const closeHover = probe('.window__control--close[data-look="hover"]');
  if (!view || !bar || !rest || !hover || !closeHover) return null;
  const look = (el: HTMLElement): Look => {
    const style = view.getComputedStyle(el);
    return { bg: style.backgroundColor, fg: style.color };
  };
  const barStyle = view.getComputedStyle(bar);
  return { bar: barStyle.backgroundColor, hairline: barStyle.borderBlockEndColor, rest: look(rest), hover: look(hover), closeHover: look(closeHover) };
}

/** Makes canvases of the page's document. A canvas with no 2D context gives null. */
export function canvasSurface(doc: Document): (width: number, height: number) => Surface | null {
  return (width, height) => {
    const canvas = doc.createElement("canvas");
    canvas.width = width;
    canvas.height = height;
    // An ordinary canvas, read once: one made for frequent reads is drawn by another rasteriser, which gives a thin diagonal
    // stroke other pixels than the page's own icon has (live check, finding 3).
    const ctx = canvas.getContext("2d");
    return ctx ? { ctx, path: (d) => new Path2D(d) } : null;
  };
}
