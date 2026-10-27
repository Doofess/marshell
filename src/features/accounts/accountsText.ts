import type { AccountStatus } from "../../generated/AccountStatus";
import { relativeTime } from "../history/historyState";

export const NONE_FOUND = "No agent CLI is installed, so there is nothing to sign in to.";
export const CHECKING = "Checking…";
export const UNREACHABLE = "Couldn't reach the Marshell core.";
export const CHECKED = "Checked the sign-in status.";

const upperFirst = (s: string) => s.charAt(0).toUpperCase() + s.slice(1);

/** One CLI's sign-in line, from what its status command reported. Unknown parts are left out. */
export function accountLine(s: AccountStatus, name: string, now: number): string {
  if (s.sign_in === "no_command") return `${name} has no sign-in status command, so Marshell can't show one.`;
  const time = relativeTime(now - s.checked_ms);
  switch (s.sign_in) {
    case "signed_out":
      return `Not signed in, checked ${time}`;
    case "unknown":
      return `Couldn't check the sign-in status, checked ${time}`;
    default: {
      const parts = [s.method ? `Signed in with ${s.method}` : s.email ? `Signed in as ${s.email}` : "Signed in"];
      if (s.plan) parts.push(upperFirst(s.plan));
      parts.push(`checked ${time}`);
      return parts.join(", ");
    }
  }
}
