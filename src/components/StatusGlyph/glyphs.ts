export type GlyphKind =
  | "idle"
  | "working"
  | "needs-permission"
  | "needs-question"
  | "done-unseen"
  | "done-seen"
  | "error"
  | "stuck"
  | "ended"
  | "limited";
export type AuxKind = "muted" | "elevated" | "caution";
export type Motion = "pulse" | "bounce" | "draw" | "flash" | null;
export type GlyphSpec = { label: string; motion: Motion; reduced: string | null };

/** docs/PLAN.md "Sidebar rows" table: shape, motion and its reduced-motion form. */
export const GLYPHS: Record<GlyphKind, GlyphSpec> = {
  idle: { label: "Idle", motion: null, reduced: null },
  working: { label: "Working", motion: "pulse", reduced: "Static dot with a three-quarter arc" },
  "needs-permission": { label: "Needs you: permission", motion: "bounce", reduced: "No bounce" },
  "needs-question": { label: "Needs you: question", motion: "bounce", reduced: "No bounce" },
  "done-unseen": { label: "Done, unseen", motion: "draw", reduced: "Check shown complete" },
  "done-seen": { label: "Done", motion: null, reduced: null },
  error: { label: "Error", motion: "flash", reduced: "No flash" },
  stuck: { label: "Might be stuck", motion: null, reduced: null },
  ended: { label: "Ended", motion: null, reduced: null },
  limited: { label: "Status limited", motion: null, reduced: null },
};

export const AUX_LABELS: Record<AuxKind, string> = {
  muted: "Muted",
  elevated: "Administrator",
  caution: "Runs without asking",
};
