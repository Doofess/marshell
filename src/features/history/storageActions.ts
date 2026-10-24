import type { RetentionLimits } from "../../generated/RetentionLimits";
import type { RetentionPreview } from "../../generated/RetentionPreview";
import type { StorageReport } from "../../generated/StorageReport";
import { failureText } from "../agents/useInstallFlow";
import { formatBytes } from "../../lib/format";
import { SettingsReadError } from "../appearance/settingsSync";

/** A change to the limits that would remove files, waiting for the person's yes. */
export type PendingLimits = { limits: RetentionLimits; preview: RetentionPreview };

const DATE = new Intl.DateTimeFormat("en-GB", { day: "numeric", month: "long", year: "numeric" });

/** The line that asks whether to remove what a change to the limits would remove. */
export function removalQuestion({ preview }: PendingLimits): string {
  const size = formatBytes(preview.bytes);
  const from = preview.oldest_ms === null ? null : DATE.format(new Date(preview.oldest_ms));
  if (preview.files === 1) return `This removes 1 recording or image now (${size})${from ? `, from ${from}` : ""}.`;
  return `This removes ${preview.files} recordings and images now (${size})${from ? `, the oldest from ${from}` : ""}.`;
}

/** Asks what `limits` would remove. Nothing: the limits are saved and null comes back. Something: the question to ask, said through `announce` too, since focus stays on the select. */
export async function proposeLimits(preview: (l: RetentionLimits) => Promise<RetentionPreview>, save: (l: RetentionLimits) => Promise<void>, limits: RetentionLimits, announce?: (text: string) => void): Promise<PendingLimits | null> {
  const found = await preview(limits);
  if (found.files === 0) {
    await save(limits);
    return null;
  }
  const ask = { limits, preview: found };
  announce?.(removalQuestion(ask));
  return ask;
}

/** The report once the cleanup a limits save asked for has run: the core runs it on its storage thread just after the save, so a report read at once still shows the files it removes. Reads again until the last cleanup is newer than `before` (its time before the save), at most 20 times, a quarter second apart; then the last report read. */
export async function reportAfterCleanup(get: () => Promise<StorageReport>, before: number | null, wait: () => Promise<void> = () => new Promise((done) => setTimeout(done, 250))): Promise<StorageReport> {
  let report = await get();
  for (let tries = 1; tries < 20 && (report.last_cleanup?.at_ms ?? null) === before; tries++) {
    await wait();
    report = await get();
  }
  return report;
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
