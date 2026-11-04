import type { LaunchProgram } from "../launcher/launchChoice";
import type { Detection } from "../onboarding/types";

/** Agent setup supplies administrator restrictions; Windows skip-every-prompt settings do not describe a WSL launch. */
export function launcherSetup(programs: LaunchProgram[], wsl: boolean, detections: Detection[] = [], detectionFailed?: ReadonlySet<string>): LaunchProgram[] {
  return programs.map((program) => {
    const agent = program.agent ?? program.id;
    const reportsSetting = agent === "antigravity" && !wsl;
    const detection = detections.find((d) => d.cli === agent);
    const failed = reportsSetting && detectionFailed?.has(agent) === true;
    return {
      ...program,
      skipsAll: wsl ? undefined : detection?.skipsAll,
      adminUnavailable: detection?.adminUnavailable,
      adminSetupPending: reportsSetting && detection === undefined && !failed,
      adminSetupFailed: failed,
    };
  });
}

/** The core reserves a session's number before the spawn, so the live list's next number moves on while a start is pending: show the one the start took. */
export function shownNumber(live: number, starting: boolean, frozen: number | undefined): number {
  return starting && frozen !== undefined ? frozen : live;
}
