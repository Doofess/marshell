import type { MenuItem } from "../../components/ContextMenu/menuKeys";

export type ViewportId = "phone" | "tablet" | "laptop" | "desktop";

/** The four sizes; the one list of them in the app (the shell keeps its own copy of the widths in Rust). */
export const VIEWPORTS: readonly { id: ViewportId; name: string; width: number }[] = [
  { id: "phone", name: "Phone", width: 390 },
  { id: "tablet", name: "Tablet", width: 768 },
  { id: "laptop", name: "Laptop", width: 1024 },
  { id: "desktop", name: "Desktop", width: 1440 },
];

export const RESPONSIVE = "Responsive";

const find = (id: ViewportId) => VIEWPORTS.find((v) => v.id === id)!;

/** A tab's size; a missing or unknown value is null (Responsive). */
export function viewportOf(tab: { viewport?: string } | undefined): ViewportId | null {
  return VIEWPORTS.find((v) => v.id === tab?.viewport)?.id ?? null;
}

export function viewportWidth(id: ViewportId | null): number | null {
  return id === null ? null : find(id).width;
}

const usable = (slot: number): boolean => Number.isFinite(slot) && slot > 0;

/** How a `width` px page sits in a `slot` px wide place: null when Responsive or the slot has no width. `scale` is 1 when it fits. */
export function viewportFit(slot: number, width: number | null): { width: number; scale: number } | null {
  if (width === null || !usable(slot)) return null;
  if (width <= slot) return { width, scale: 1 };
  return { width, scale: Math.round((slot / width) * 10000) / 10000 };
}

/** Where a fixed-width page sits in its `slot` px wide place, exactly as the shell's `fit` places it: undefined when Responsive and when the page doesn't fit, where the page is the whole place. */
export function pageBox(slot: number, width: number | null): { inlineSize: number; marginInlineStart: number } | undefined {
  if (width === null || !usable(slot) || width >= slot) return undefined;
  return { inlineSize: width, marginInlineStart: Math.floor((slot - width) / 2) };
}

export function viewportRows(now: ViewportId | null): MenuItem[] {
  return [
    { id: "viewport:responsive", label: RESPONSIVE, radio: true, checked: now === null },
    ...VIEWPORTS.map((v): MenuItem => ({ id: `viewport:${v.id}`, label: `${v.name} (${v.width} px)`, radio: true, checked: now === v.id })),
  ];
}

/** The size a viewport row chooses: null for Responsive, undefined for an id that is not a viewport row. */
export function viewportChoice(id: string): ViewportId | null | undefined {
  if (id === "viewport:responsive") return null;
  return VIEWPORTS.find((v) => `viewport:${v.id}` === id)?.id;
}

const shownAt = (id: ViewportId, slot: number): number | null => {
  const fit = viewportFit(slot, find(id).width);
  return fit && fit.scale < 1 ? Math.round(fit.scale * 100) : null;
};

export function chipText(id: ViewportId, slot: number): string {
  const v = find(id);
  const pct = shownAt(id, slot);
  return `${v.name} ${v.width}${pct === null ? "" : ` · ${pct}%`}`;
}

export function chipLabel(id: ViewportId, slot: number): string {
  const v = find(id);
  const pct = shownAt(id, slot);
  return `Viewport: ${v.name}, ${v.width} pixels wide${pct === null ? "" : `, shown at ${pct}%`}`;
}

export const viewportSaid = (id: ViewportId | null): string => `Viewport: ${id === null ? RESPONSIVE : find(id).name}`;

/** The style of an HTML file's frame in a box: undefined when Responsive. */
export function frameBox(
  box: { width: number; height: number },
  width: number | null,
): { inlineSize: number; blockSize?: number; marginInline?: "auto"; transform?: string; transformOrigin?: "0 0" } | undefined {
  const fit = viewportFit(box.width, width);
  if (!fit) return undefined;
  if (fit.scale === 1) return { inlineSize: fit.width, marginInline: "auto" };
  return { inlineSize: fit.width, blockSize: box.height / fit.scale, transform: `scale(${fit.scale})`, transformOrigin: "0 0" };
}
