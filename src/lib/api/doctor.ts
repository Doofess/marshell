import type { ApplyRequest } from "../../generated/ApplyRequest";
import type { ApplyResult } from "../../generated/ApplyResult";
import type { ConfigPreview } from "../../generated/ConfigPreview";
import type { DoctorFixPlan } from "../../generated/DoctorFixPlan";
import type { DoctorLastFix } from "../../generated/DoctorLastFix";
import type { DoctorReport } from "../../generated/DoctorReport";
import type { DoctorSummary } from "../../generated/DoctorSummary";
import type { Endpoint } from "../../generated/Endpoint";
import { conflictBody } from "./agents";
import { api, ApiError } from "./client";

/** Starts a check. The answer is the summary at that moment; the finished one arrives as a `doctor` frame. */
export const runDoctor = (ep: Endpoint) => api<DoctorSummary>(ep, "POST", "/v1/doctor/run");

/** The last finished report, or null before the first run ends. */
export const doctorReport = async (ep: Endpoint): Promise<DoctorReport | null> => (await api<DoctorReport | undefined>(ep, "GET", "/v1/doctor/report")) ?? null;

/** The one preview that fixes these items. A 422 ApiError says an item can't be fixed any more. */
export const planFix = (ep: Endpoint, ids: readonly string[]) => api<DoctorFixPlan>(ep, "POST", "/v1/doctor/fix", { item_ids: ids });

export type FixOutcome = { ok: true; fix: DoctorLastFix } | { ok: false; conflict: true } | { ok: false; conflict: false; message: string };

/** Applies the fix preview, guarded by each file's hash. Codes map as `applyPreview` maps them: 404 and a changed 409 are conflicts, a refusal is not. */
export async function applyFix(ep: Endpoint, preview: ConfigPreview): Promise<FixOutcome> {
  const body: ApplyRequest = {
    changeset_id: preview.changeset_id,
    expected_hashes: preview.files.map((f) => (f.before_hash === undefined ? { path: f.path } : { path: f.path, hash: f.before_hash })),
  };
  try {
    return { ok: true, fix: await api<DoctorLastFix>(ep, "POST", "/v1/doctor/fix/apply", body) };
  } catch (e) {
    if (e instanceof ApiError) {
      if (e.status === 404) return { ok: false, conflict: true };
      if (e.status !== 409) return { ok: false, conflict: false, message: e.message };
      const { reason, paths } = conflictBody(e.message);
      if (reason !== "refused") return { ok: false, conflict: true };
      return { ok: false, conflict: false, message: paths.length > 0 ? `Marshell can't change ${paths.join(", ")}, so nothing was written.` : "Nothing was written: there is nothing Marshell can change here." };
    }
    throw e;
  }
}

/** The record of the last fix, or null when there is none. */
export const lastFix = async (ep: Endpoint): Promise<DoctorLastFix | null> => (await api<DoctorLastFix | undefined>(ep, "GET", "/v1/doctor/undo")) ?? null;

export type UndoOutcome = { ok: true } | { ok: false; changed: string[] } | { ok: false; message: string };

/** Puts the last fix's backup back. A 409 `changed` lists the files that changed since the fix, and nothing was written. */
export async function undoFix(ep: Endpoint): Promise<UndoOutcome> {
  try {
    const done = await api<ApplyResult | undefined>(ep, "POST", "/v1/doctor/undo");
    return done === undefined ? { ok: false, message: "There is nothing to undo." } : { ok: true };
  } catch (e) {
    if (!(e instanceof ApiError)) throw e;
    if (e.status === 409) {
      const { reason, paths } = conflictBody(e.message);
      if (reason === "changed") return { ok: false, changed: paths };
    }
    return { ok: false, message: e.message };
  }
}

/** What the files say now against the backup: a preview only, nothing is written. */
export const undoDifferences = (ep: Endpoint, snapshotId: string) => api<ConfigPreview>(ep, "POST", `/v1/snapshots/${encodeURIComponent(snapshotId)}/restore`);
