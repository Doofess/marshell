import type { DoctorAction } from "../../generated/DoctorAction";
import type { DoctorSummary } from "../../generated/DoctorSummary";

/** The button's visible text: short, and busy while its request runs. */
export const keepLabel = (action: DoctorAction, busy: boolean) => {
  if (action === "useCopy") return busy ? "Preparing…" : "Use this copy's text";
  return action === "keep" ? (busy ? "Keeping…" : "Keep my file") : busy ? "Stopping…" : "Stop keeping";
};
/** The button's accessible name: its visible text first, then the row it belongs to. */
export const keepName = (action: DoctorAction, title: string, busy: boolean) => `${keepLabel(action, busy)}: ${title}`;

/** What the announcer says once a press went through. */
export const keepDone = (action: DoctorAction) => (action === "keep" ? "Marshell keeps your file as it is." : "Marshell no longer keeps that file.");
/**
 * The mark both of one file's rows carry on their button ("isn't Marshell's file any more" and "has your changes"):
 * a press swaps one row for the other, and the focus follows to the button that took its place. A "Use this copy's
 * text" row carries its own: it swaps with nothing, and a Keep press on the same CLI must not land on it.
 */
export const keepFocusKey = (item: { agent?: string | null; distro?: string | null; action?: DoctorAction | null; id?: string }) =>
  item.action === "useCopy" ? `use:${item.id ?? ""}` : `${item.agent ?? ""}|${item.distro ?? ""}`;

/** Whether a press left the focus nowhere: on nothing, on the body, or on the page itself, which takes it while a check hides the rows. */
export const keepFocusLost = (active: unknown, body: unknown, page: unknown) => active === null || active === body || (page !== null && active === page);

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
