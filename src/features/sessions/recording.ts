import type { Endpoint } from "../../generated/Endpoint";
import { sessions } from "../../lib/api/sessions";

/** The palette's "Copy this session's recording path": puts the .cast file's path on the clipboard and says how it went. */
export async function copyRecordingPath(endpoint: Endpoint, tabId: string, announce: (text: string) => void): Promise<void> {
  try {
    await navigator.clipboard.writeText(await sessions.recordingPath(endpoint, tabId));
    announce("Copied the recording's path.");
  } catch (e) {
    announce(`Couldn't copy the recording's path: ${e instanceof Error ? e.message : String(e)}`);
  }
}
