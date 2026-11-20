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
 * Edges are rounded from the scale, not each width, so the three pictures add up to the strip's width at a fractional scale
 * (at 1.25 they are 58, 57 and 58 wide, not three equal ones that would leave a gap or overlap).
 */
export function tileLayout(dpr: number): TileLayout {
  const edge = (i: number) => Math.round(i * BUTTON_W * dpr);
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
  // The glyph is placed in CSS pixels and centred in the rows above the hairline, as the page's own button centres it.
  ctx.setTransform(dpr, 0, 0, dpr, -left, 0);
  ctx.translate(slot * BUTTON_W + (BUTTON_W - GLYPH) / 2, (BAR_H - 1 - GLYPH) / 2);
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
 * as written, which a canvas understands; in forced colours they already hold the system's colours. The hover look comes from the
 * two hidden probe buttons, which take the hover rules by their `data-look` attribute, so the pointer need not be on a button.
 */
export function readLooks(controls: HTMLElement): StripLooks | null {
  const view = controls.ownerDocument.defaultView;
  const bar = controls.closest<HTMLElement>(".window__bar");
  const minimize = controls.querySelector<HTMLElement>(".window__control");
  const probes = controls.parentElement?.querySelectorAll<HTMLElement>('[data-look="hover"]');
  const hover = probes?.[0];
  const closeHover = probes?.[1];
  if (!view || !bar || !minimize || !hover || !closeHover) return null;
  const look = (el: HTMLElement): Look => {
    const style = view.getComputedStyle(el);
    return { bg: style.backgroundColor, fg: style.color };
  };
  const barStyle = view.getComputedStyle(bar);
  return { bar: barStyle.backgroundColor, hairline: barStyle.borderBlockEndColor, rest: look(minimize), hover: look(hover), closeHover: look(closeHover) };
}

/** Makes canvases of the page's document. A canvas with no 2D context gives null. */
export function canvasSurface(doc: Document): (width: number, height: number) => Surface | null {
  return (width, height) => {
    const canvas = doc.createElement("canvas");
    canvas.width = width;
    canvas.height = height;
    const ctx = canvas.getContext("2d", { willReadFrequently: true });
    return ctx ? { ctx, path: (d) => new Path2D(d) } : null;
  };
}
