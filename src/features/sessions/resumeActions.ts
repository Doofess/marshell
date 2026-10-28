import type { ResumeForm } from "../../generated/ResumeForm";
import type { ResumeCommandResult, Started } from "../../lib/api/sessions";

/** What the toast says after copying, by which form of the command was copied (slice 6h1 decision 12). */
export function toastFor(form: ResumeForm): string {
  switch (form) {
    case "by_id":
      return "Copied the resume command.";
    case "continue_last":
      return "Copied. It continues the last conversation in this folder.";
    case "fresh":
      return "Copied. It starts a new conversation: this one was never saved.";
  }
}

export const ENDED_FIRST = "End the session first.";

const failure = (what: string, e: unknown) => `Couldn't ${what}: ${e instanceof Error ? e.message : String(e)}`;

/** "Copy resume command", for a session row or an archived conversation: copies the core's line and says which form it was; a refusal is announced in the core's words. */
export async function copyResumeCommand(
  get: () => Promise<ResumeCommandResult>,
  write: (text: string) => Promise<void>,
  announce: (text: string) => void,
): Promise<void> {
  try {
    const r = await get();
    if (!r.ok) return announce(r.reason);
    await write(r.command);
    announce(toastFor(r.form));
  } catch (e) {
    announce(failure("copy the resume command", e));
  }
}

/** "Open in system terminal": the core opens it, and the toast says so or gives the core's reason. */
export async function openInSystemTerminal(call: () => Promise<Started>, announce: (text: string) => void): Promise<void> {
  try {
    const r = await call();
    announce(r.ok ? "Opened in a system terminal." : r.reason);
  } catch (e) {
    announce(failure("open a system terminal", e));
  }
}
