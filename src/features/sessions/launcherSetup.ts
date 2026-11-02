import type { LaunchProgram } from "../launcher/launchChoice";
import type { Detection } from "../onboarding/types";

/** Only Antigravity reports a setting that skips every prompt; Windows detections never describe a WSL launch. */
export function launcherSetup(programs: LaunchProgram[], wsl: boolean, detections: Detection[] = [], detectionFailed?: ReadonlySet<string>): LaunchProgram[] {
  return programs.map((program) => {
    const agent = program.agent ?? program.id;
    const reportsSetting = agent === "antigravity" && !wsl;
    const detection = reportsSetting ? detections.find((d) => d.cli === agent) : undefined;
    const failed = reportsSetting && detectionFailed?.has(agent) === true;
    return {
      ...program,
      skipsAll: detection?.skipsAll,
      adminSetupPending: reportsSetting && detection === undefined && !failed,
      adminSetupFailed: failed,
    };
  });
}
