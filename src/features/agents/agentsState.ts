import type { AgentSetup } from "../../generated/AgentSetup";
import type { ConfigPreview } from "../../generated/ConfigPreview";
import { previewDiff, toDetection } from "../../lib/api/agents";
import { addsStatusLine, consentHeadline, removalCopy, type Capability, type ConsentPlan, type Detection } from "../onboarding/consent";

/** The switches the card shows for Claude Code until 3d: status, and the status line. */
export const OFFER: Capability[] = ["status", "statusline"];
/** What the person has switched on. The approve switch comes with 3d. */
export type Want = { status: boolean; statusline: boolean };
export const DEFAULT_WANT: Want = { status: true, statusline: true };
/** The body of the install request. Status off means nothing is installed, so the status line is never asked for either. */
export const installRequest = (w: Want): Want => ({ status: w.status, statusline: w.status && w.statusline });
const onOf = (w: Want): Record<Capability, boolean> => ({ status: w.status, approve: false, statusline: w.status && w.statusline });
const ON = onOf(DEFAULT_WANT);

/** The setup's file does not parse: there is nothing to set up or turn off until it is fixed. */
const refused = (s: AgentSetup) => s.settings_state === "unparsable";

/** One line on what Marshell's hooks are doing in Claude Code's settings file. */
export function setupLine(s: AgentSetup): string {
  if (!s.found) return "Claude Code was not found on this computer.";
  if (refused(s)) return consentHeadline(toDetection(s), ON);
  switch (s.hooks) {
    case "none":
      return "Not set up. Sessions show limited status.";
    case "installed":
      return `Hooks on in ${s.settings_path}. New sessions report status.`;
    case "partial":
      return `Some hooks are missing in ${s.settings_path}. Set up again to repair them.`;
    case "orphaned":
      return `Marshell's hooks are in ${s.settings_path}, but the bridge at ${s.bridge_path} is missing, so Claude Code reports a failing hook each time one fires. Turn the hooks off, or set up again.`;
  }
}

/** What the person may do about it. Orphaned hooks can be turned off or set up again. */
export function actions(s: AgentSetup): Array<"setup" | "hooksOff"> {
  if (!s.found || refused(s)) return [];
  switch (s.hooks) {
    case "none":
      return ["setup"];
    case "installed":
      return ["hooksOff"];
    case "partial":
    case "orphaned":
      return ["setup", "hooksOff"];
  }
}

/** The notice after the hooks are turned off. */
export const hooksOffNotice = (statuslineLeft: boolean): string =>
  `Hooks off. Sessions started from now show limited status.${statuslineLeft ? " Your status line was changed since setup, so it was left as it is." : ""}`;

/** Where a Set up is: the preview the core made (none when the file is refused), and what the card says about it. */
export type InstallFlow = { preview: ConfigPreview | null; open: boolean; busy: boolean; error?: string; conflict?: boolean; want?: Want };

/** The consent card's input for the core's setup and preview. A refused file gets a headline that says nothing will change. */
export function consentProps(s: AgentSetup, flow: InstallFlow) {
  const want = flow.want ?? DEFAULT_WANT;
  const on = onOf(want);
  const detection: Detection = toDetection(s);
  const files = flow.preview?.files ?? [];
  const chain = files.find((f) => /statusline-chain\.json$/.test(f.path));
  const plan: ConsentPlan = {
    cli: "claude",
    on,
    configPath: s.settings_path,
    backupPath: s.snapshots_dir,
    removal: removalCopy("claude", on, undefined, undefined, addsStatusLine(detection, on)),
    diff: flow.preview ? previewDiff(flow.preview) : "",
    needsTrust: false,
    chainPath: chain?.path,
    headline: flow.preview ? undefined : !want.status ? "Nothing will change: status is off." : flow.error ? `Marshell will not edit ${s.settings_path}, so nothing will change.` : `Checking ${s.settings_path}…`,
  };
  return { scenario: { detection, plan }, on, offer: OFFER, open: flow.open, busy: flow.busy, error: flow.error };
}
