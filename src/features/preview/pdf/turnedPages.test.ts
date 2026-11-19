import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";

// A page with /Rotate is drawn turned, but pdf.js lays its text and note layers out upright and marks them with
// `data-main-rotation`, leaving the turn to the stylesheet. Without it a turned page's text and notes sat where the page
// would be unturned: selection, find and notes all landed beside the drawn words (live check 6q-6).
const read = (path: string) => readFileSync(new URL(path, import.meta.url), "utf8").replace(/\/\*[\s\S]*?\*\//g, "");
const pdfCss = read("./PdfView.css");
const notesCss = read("./notes/notes.css");

type M = [number, number, number, number, number, number];
const times = (m: M, n: M): M => [m[0] * n[0] + m[2] * n[1], m[1] * n[0] + m[3] * n[1], m[0] * n[2] + m[2] * n[3], m[1] * n[2] + m[3] * n[3], m[0] * n[4] + m[2] * n[5] + m[4], m[1] * n[4] + m[3] * n[5] + m[5]];

/** A CSS transform list as one matrix, for a box of `w` by `h` (percentages are of the box's own size). */
function matrixOf(transform: string, w: number, h: number): M {
  let m: M = [1, 0, 0, 1, 0, 0];
  for (const [, fn, args] of transform.matchAll(/([a-zA-Z]+)\(([^)]*)\)/g)) {
    const a = args!.split(/\s*,\s*|\s+/).filter(Boolean);
    const len = (s: string | undefined, of: number) => (s === undefined ? 0 : s.endsWith("%") ? (parseFloat(s) / 100) * of : parseFloat(s));
    let n: M;
    if (fn === "rotate") {
      const r = (parseFloat(a[0]!) * Math.PI) / 180;
      n = [Math.cos(r), Math.sin(r), -Math.sin(r), Math.cos(r), 0, 0];
    } else if (fn === "translateX") n = [1, 0, 0, 1, len(a[0], w), 0];
    else if (fn === "translateY") n = [1, 0, 0, 1, 0, len(a[0], h)];
    else if (fn === "translate") n = [1, 0, 0, 1, len(a[0], w), len(a[1], h)];
    else if (fn === "matrix") n = a.map(Number) as M;
    else throw new Error(`no ${fn}`);
    m = times(m, n);
  }
  return m;
}
const at = (m: M, x: number, y: number) => [m[0] * x + m[2] * y + m[4], m[1] * x + m[3] * y + m[5]].map((v) => Math.round(v * 1000) / 1000 + 0);

/** The transform of the first rule whose selector holds every one of `parts`. */
function transformFor(css: string, parts: string[]): string | null {
  for (const [, selector, body] of css.matchAll(/([^{}]+)\{([^{}]*)\}/g)) {
    if (parts.every((p) => selector!.includes(p))) return /transform:\s*([^;]+);/.exec(body!)?.[1]?.trim() ?? null;
  }
  return null;
}

const W = 612;
const H = 792;
// Where the page's own top-left corner is drawn once the page is turned clockwise by each angle.
const cornerAt: Record<number, [number, number]> = { 90: [H, 0], 180: [W, H], 270: [0, W] };

describe("turned pages: the text and note layers lie over the drawn page", () => {
  for (const r of [90, 180, 270]) {
    it(`turned_page_layers_cover_the_drawn_page_${r}`, () => {
      const t = transformFor(pdfCss, [".textLayer", ".annotationEditorLayer", `[data-main-rotation="${r}"]`]);
      expect(t, `a rule for both layers at ${r}`).not.toBeNull();
      const m = matrixOf(t!, W, H);
      const corners = [at(m, 0, 0), at(m, W, 0), at(m, 0, H), at(m, W, H)];
      const xs = corners.map((c) => c[0]!);
      const ys = corners.map((c) => c[1]!);
      const [w2, h2] = r % 180 === 0 ? [W, H] : [H, W];
      expect([Math.min(...xs), Math.min(...ys), Math.max(...xs), Math.max(...ys)]).toEqual([0, 0, w2, h2]);
      expect(at(m, 0, 0)).toEqual(cornerAt[r]);
    });
  }
  it("turned_page_layers_turn_about_their_top_left_corner", () => {
    expect(notesCss).toMatch(/\.annotationEditorLayer\s*\{[^}]*transform-origin:\s*0 0/);
    expect(pdfCss).toMatch(/\.textLayer\s*\{[^}]*transform-origin:\s*0 0/);
  });
});

describe("turned pages: a note turns about its own corner", () => {
  // pdf.js places a turned note from its corner and turns it there; a shift as well puts it a box's length away.
  for (const r of [90, 180, 270]) {
    it(`a_turned_note_turns_without_moving_${r}`, () => {
      const t = transformFor(notesCss, [`[data-editor-rotation="${r}"]`]);
      expect(t).not.toBeNull();
      const m = matrixOf(t!, 100, 20);
      expect(at(m, 0, 0)).toEqual([0, 0]);
      const rad = (r * Math.PI) / 180;
      expect(at(m, 100, 0)).toEqual(at([Math.cos(rad), Math.sin(rad), 0, 0, 0, 0], 100, 0));
    });
  }
});

describe("turned pages: a highlight is drawn turned with its page", () => {
  // The draw layer holds each highlight in a 1 by 1 box of the upright page; on a turned page the shape turns inside it.
  for (const r of [90, 180, 270]) {
    it(`a_highlight_s_shape_turns_with_the_page_${r}`, () => {
      const t = transformFor(notesCss, ["svg", ".highlight", `[data-main-rotation="${r}"]`, "use"]);
      expect(t).not.toBeNull();
      const m = matrixOf(t!, 1, 1);
      expect(at(m, 0, 0)).toEqual({ 90: [1, 0], 180: [1, 1], 270: [0, 1] }[r]);
      expect(at(m, 1, 0)).toEqual({ 90: [1, 1], 180: [0, 1], 270: [0, 0] }[r]);
    });
  }
});
