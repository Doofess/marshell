import type { AccountStatus } from "../../generated/AccountStatus";
import { relativeTime } from "../history/historyState";

export const NONE_FOUND = "No agent CLI is installed, so there is nothing to sign in to.";
export const CHECKING = "Checking…";
export const UNREACHABLE = "Couldn't reach the Marshell core.";
export const CHECKED = "Checked the sign-in status.";

const upperFirst = (s: string) => s.charAt(0).toUpperCase() + s.slice(1);

const bareHost = (h: string) => h.replace(/^https:\/\//, "").replace(/\/$/, "");

/**
 * The lines under the first. Qwen Code's settings name the sign-in type they choose (the core sends its label as `method`).
 * Copilot CLI's own file names the last account (`last_account` is "login" or "login on host"), then a set token variable's name.
 */
export function accountExtraLines(s: AccountStatus): string[] {
  if (s.sign_in !== "no_command") return [];
  if (s.agent === "qwen") return s.method ? [`Its settings choose ${s.method}.`] : [];
  if (s.agent !== "copilot") return [];
  const lines: string[] = [];
  if (s.last_account) {
    const at = s.last_account.indexOf(" on ");
    const login = at < 0 ? s.last_account : s.last_account.slice(0, at);
    const host = at < 0 ? "" : s.last_account.slice(at + 4);
    const onHost = host !== "" && bareHost(host) !== "github.com" ? ` on ${host}` : "";
    lines.push(`Copilot's own file names ${login}${onHost} as the last account signed in.`);
  }
  if (s.token_env) lines.push(`${s.token_env} is set, and Copilot CLI uses it before a stored sign-in.`);
  return lines;
}

/** One CLI's sign-in line, from what its status command reported. Unknown parts are left out. */
export function accountLine(s: AccountStatus, name: string, now: number): string {
  if (s.sign_in === "no_command" && s.agent === "qwen") return `${name} has no sign-in status command, so Marshell can't show one. Sign in with /auth in a Qwen Code session.`;
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
