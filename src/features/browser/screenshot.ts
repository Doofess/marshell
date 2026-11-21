// "Save screenshot": the shell takes the page's picture, the core saves it to Downloads and names the file.
import { ApiError } from "../../lib/api/client";

const two = (n: number) => String(n).padStart(2, "0");

/** What the core names the file from: the page's host with its port ("localhost:5173"; "" for an address without one) and the local time as "2026-11-21 134501". */
export function shotAsk(url: string, at: Date): { host: string; at: string } {
  let host = "";
  try {
    host = new URL(url).host;
  } catch {
    host = "";
  }
  const day = `${at.getFullYear()}-${two(at.getMonth() + 1)}-${two(at.getDate())}`;
  return { host, at: `${day} ${two(at.getHours())}${two(at.getMinutes())}${two(at.getSeconds())}` };
}

export type ShotTarget = { kind: "page"; key: string; url: string; loaded: boolean } | { kind: "file" } | { kind: "none" };

/** Why a picture can't be taken now; null when it can. */
export function shotReason(t: ShotTarget, pages: boolean, panelShown: boolean): string | null {
  if (!pages) return "Not available on this system yet";
  if (t.kind === "file") return "Not for an HTML file";
  if (t.kind === "none") return "No page is open";
  if (!t.loaded) return "The page isn't loaded";
  if (!panelShown) return "Show the Browser panel to save a screenshot.";
  return null;
}

export const savedText = (name: string, cut: boolean): string =>
  cut ? `Saved the first 16,384 pixels of the page as ${name} to Downloads.` : `Saved ${name} to Downloads.`;

export const SHOT_FAILED = "The screenshot wasn't saved.";
export const SHOT_COVERED = "The page is covered. Close what covers it and try again.";
const SHOT_HIDDEN = "The page isn't on screen. Show it and try again.";
export const COVER_WAIT_MS = 5000;

export type ShotDeps = {
  uncovered(limitMs: number): Promise<boolean>;
  paint(): Promise<void>;
  shoot(key: string, full: boolean): Promise<{ png: string; cut: boolean }>;
  save(host: string, at: string, png: Blob): Promise<{ name: string }>;
  now(): Date;
};

function pngBlob(base64: string): Blob {
  const raw = atob(base64);
  const bytes = new Uint8Array(raw.length);
  for (let i = 0; i < raw.length; i++) bytes[i] = raw.charCodeAt(i);
  return new Blob([bytes], { type: "image/png" });
}

/** Takes and saves one picture; resolves to the line to announce. Never rejects. */
export async function saveShot(deps: ShotDeps, t: { key: string; url: string }, full: boolean): Promise<string> {
  try {
    if (!(await deps.uncovered(COVER_WAIT_MS))) return SHOT_COVERED;
    // Two paints: the uncover effect has then queued the page's show in browserApi's queue, which the shot waits for.
    await deps.paint();
    await deps.paint();
    const shot = await deps.shoot(t.key, full);
    const ask = shotAsk(t.url, deps.now());
    const saved = await deps.save(ask.host, ask.at, pngBlob(shot.png));
    return savedText(saved.name, shot.cut);
  } catch (error) {
    if (error === "hidden") return SHOT_HIDDEN;
    if (error instanceof ApiError && error.message) return `${SHOT_FAILED} ${error.message}`;
    return SHOT_FAILED;
  }
}
