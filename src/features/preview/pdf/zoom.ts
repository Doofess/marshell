export const ZOOM_STEPS = [0.5, 0.75, 1, 1.25, 1.5, 2, 3];

export type Zoom = { kind: "fit" } | { kind: "scale"; value: number };

/** The scale a page is drawn at: "fit" fills the width that is available, a fixed zoom is its own value. */
export function scaleFor(zoom: Zoom, pageWidth1: number, available: number): number {
  if (zoom.kind === "scale") return zoom.value;
  return pageWidth1 > 0 && available > 0 ? available / pageWidth1 : 1;
}

const EPS = 0.005;

/** The next stop above the scale in view (from "fit" too), or the top stop. */
export function zoomIn(z: Zoom, current: number): Zoom {
  const from = z.kind === "scale" ? z.value : current;
  const next = ZOOM_STEPS.find((s) => s > from + EPS);
  return { kind: "scale", value: next ?? ZOOM_STEPS[ZOOM_STEPS.length - 1]! };
}

/** The next stop below the scale in view, or the bottom stop. */
export function zoomOut(z: Zoom, current: number): Zoom {
  const from = z.kind === "scale" ? z.value : current;
  const prev = [...ZOOM_STEPS].reverse().find((s) => s < from - EPS);
  return { kind: "scale", value: prev ?? ZOOM_STEPS[0]! };
}

/** A page number typed into the page field: whole, and within 1..count; anything else is null. */
export function parsePageField(text: string, count: number): number | null {
  const t = text.trim();
  if (!/^\d+$/.test(t)) return null;
  const n = Number(t);
  return n >= 1 && n <= count ? n : null;
}
