import type { Mode, Usage } from "./types";

const DASH = "–";

export function formatDuration(ms?: number): string {
  if (ms === undefined || !Number.isFinite(ms) || ms < 0) return DASH;
  const s = Math.floor(ms / 1000);
  if (s < 60) return `${s}s`;
  const m = Math.floor(s / 60);
  if (m < 60) return `${m}m`;
  const h = Math.floor(m / 60);
  if (h < 24) return `${h}h ${String(m % 60).padStart(2, "0")}m`;
  return `${Math.floor(h / 24)}d ${h % 24}h`;
}

export function formatTokens(n?: number): string {
  if (n === undefined || !Number.isFinite(n) || n < 0) return DASH;
  if (n < 1000) return String(Math.round(n));
  if (n < 999_500) return `${Math.round(n / 1000)}k`;
  const m = n / 1_000_000;
  return `${m < 10 ? m.toFixed(1).replace(/\.0$/, "") : Math.round(m)}M`;
}

const usd = new Intl.NumberFormat("en-US", { style: "currency", currency: "USD", minimumFractionDigits: 2 });

export function formatCost(n?: number): string {
  if (n === undefined || !Number.isFinite(n) || n < 0) return DASH;
  return usd.format(n);
}

/** The usage line of an expanded row, or null when nothing about the session's usage is known (the row then shows none). */
export function formatUsage(u?: Usage): string | null {
  if (!u || Object.values(u).every((v) => v === undefined)) return null;
  return `${formatTokens(u.inContext)} / ${formatTokens(u.window)} · ${formatTokens(u.tokensIn)} in · ${formatTokens(u.tokensOut)} out · ${formatCost(u.costUsd)}`;
}

/** Modes that run without asking always show a caution (docs/PLAN.md "Sidebar rows"). */
export function modeCaution(mode?: Mode): string | null {
  switch (mode) {
    case "auto-edit":
      return "Auto-accept edits";
    case "full-auto":
      return "Full auto";
    case "bypass":
      return "Bypass permissions";
    default:
      return null;
  }
}
