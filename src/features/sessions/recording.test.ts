import { afterEach, describe, expect, it, vi } from "vitest";
import { sessions } from "../../lib/api/sessions";
import { copyRecordingPath } from "./recording";

const ep = { port: 1, token: "t" } as unknown as Parameters<typeof copyRecordingPath>[0];

afterEach(() => {
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
});

describe("copyRecordingPath", () => {
  it("copies the path and says so", async () => {
    vi.spyOn(sessions, "recordingPath").mockResolvedValue("C:\\rec\\t-1.cast");
    const writeText = vi.fn().mockResolvedValue(undefined);
    vi.stubGlobal("navigator", { clipboard: { writeText } });
    const announce = vi.fn();
    await copyRecordingPath(ep, "t", announce);
    expect(writeText).toHaveBeenCalledWith("C:\\rec\\t-1.cast");
    expect(announce).toHaveBeenCalledWith("Copied the recording's path.");
  });
  it("gives the reason when there is no recording", async () => {
    vi.spyOn(sessions, "recordingPath").mockRejectedValue(new Error("This session has no recording."));
    vi.stubGlobal("navigator", { clipboard: { writeText: vi.fn() } });
    const announce = vi.fn();
    await copyRecordingPath(ep, "t", announce);
    expect(announce).toHaveBeenCalledWith("Couldn't copy the recording's path: This session has no recording.");
  });
});
