import type { DoctorAction } from "../../generated/DoctorAction";
import type { DoctorSummary } from "../../generated/DoctorSummary";

/** The button's visible text: short, and busy while its request runs. */
export const keepLabel = (action: DoctorAction, busy: boolean) => (action === "keep" ? (busy ? "Keeping…" : "Keep my file") : busy ? "Stopping…" : "Stop keeping");
/** The button's accessible name: its visible text first, then the row it belongs to. */
export const keepName = (action: DoctorAction, title: string, busy: boolean) => `${keepLabel(action, busy)}: ${title}`;

export type KeepOutcome = { ok: true } | { ok: false; message: string };

/**
 * Sends the press and follows the run the core asked for the way a run started here is followed: a run that is already
 * over has its report loaded now, one still going is picked up by its `doctor` frame.
 */
export async function pressKeep(start: () => Promise<DoctorSummary>, load: () => Promise<void>): Promise<KeepOutcome> {
  try {
    const s = await start();
    if (!s.running) await load();
    return { ok: true };
  } catch (e) {
    const message = e instanceof Error ? e.message : "";
    return { ok: false, message: message || "Marshell couldn't change that." };
  }
}
