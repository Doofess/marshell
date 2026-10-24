import type { RetentionLimits } from "../../generated/RetentionLimits";
import type { RetentionPreview } from "../../generated/RetentionPreview";
import { failureText } from "../agents/useInstallFlow";
import { SettingsReadError } from "../appearance/settingsSync";

/** A change to the limits that would remove files, waiting for the person's yes. */
export type PendingLimits = { limits: RetentionLimits; preview: RetentionPreview };

/** Asks what `limits` would remove. Nothing: the limits are saved and null comes back. Something: the question to ask. */
export async function proposeLimits(preview: (l: RetentionLimits) => Promise<RetentionPreview>, save: (l: RetentionLimits) => Promise<void>, limits: RetentionLimits): Promise<PendingLimits | null> {
  const found = await preview(limits);
  if (found.files === 0) {
    await save(limits);
    return null;
  }
  return { limits, preview: found };
}

/** Opens the folder picker at `current` and saves the choice. False when the person cancelled. */
export async function chooseBackupFolder(pick: (start: string | null) => Promise<string | null>, save: (dir: string) => Promise<void>, current: string | null): Promise<boolean> {
  const dir = await pick(current);
  if (dir === null) return false;
  await save(dir);
  return true;
}

/** The text for a failed read or save: the core's own words for a refusal (a 400's body), a plain line otherwise. */
export const storageErrorText = (e: unknown): string => (e instanceof SettingsReadError ? e.message : failureText(e));
