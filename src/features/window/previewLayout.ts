/** Below this width of the main area the preview takes all of it instead of sitting beside the session (docs/PLAN.md, Phase 2.5). */
export const PREVIEW_MIN_SIDE_BY_SIDE = 720;

export const PREVIEW_RATIO = { min: 0.3, max: 0.7, default: 0.5, step: 0.02 } as const;

export type PreviewLayout = "side-by-side" | "full";

export function previewLayout(mainWidth: number): PreviewLayout {
  return mainWidth >= PREVIEW_MIN_SIDE_BY_SIDE ? "side-by-side" : "full";
}

/** The session's share of the main area, held between 30% and 70%. */
export function clampPreviewRatio(ratio: number): number {
  return Math.min(PREVIEW_RATIO.max, Math.max(PREVIEW_RATIO.min, ratio));
}

/** Grid columns for the session, the divider's hairline and the preview. */
export function previewTracks(ratio: number): string {
  const r = clampPreviewRatio(ratio);
  const rest = Math.round((1 - r) * 1000) / 1000;
  return `minmax(0, ${r}fr) 1px minmax(0, ${rest}fr)`;
}
