import { describe, expect, it } from "vitest";
import { AGY_SETUP, SETUP } from "../agents/fixtures";
import { recordSetupDetection, type SetupDetections } from "./setupDetections";

const empty = (): SetupDetections => ({ detections: [], detectionFailed: new Set() });

describe("setup detection results", () => {
  it("records a failed read and removes the agent's stale detection", () => {
    const known = recordSetupDetection(recordSetupDetection(empty(), "claude", SETUP), "antigravity", { ...AGY_SETUP, skips_all: "dangerouslySkipPermissions" });
    const failed = recordSetupDetection(known, "antigravity");
    expect(failed.detectionFailed.has("antigravity")).toBe(true);
    expect(failed.detections.map((d) => d.cli)).toEqual(["claude"]);
    expect(known.detectionFailed.size).toBe(0);
    expect(known.detections).toHaveLength(2);
  });

  it("a later successful read clears the failure and reports the current setting", () => {
    const failed = recordSetupDetection(empty(), "antigravity");
    const recovered = recordSetupDetection(failed, "antigravity", AGY_SETUP);
    expect(recovered.detectionFailed.has("antigravity")).toBe(false);
    expect(recovered.detections).toMatchObject([{ cli: "antigravity" }]);
    expect(recovered.detections[0]?.skipsAll).toBeUndefined();
    expect(failed.detectionFailed.has("antigravity")).toBe(true);
  });
});
