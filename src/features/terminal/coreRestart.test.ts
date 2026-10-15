import { describe, expect, it } from "vitest";
import { ENDED_LINE, FAILED_LINE } from "./coreRestart";

describe("core restart lines", () => {
  it("says the session ended and how to get a new shell", () => {
    expect(ENDED_LINE).toBe("\r\n\x1b[2mMarshell's core restarted, so this session ended. Press Enter for a new shell.\x1b[0m\r\n");
  });

  it("says the core could not restart and what to do", () => {
    expect(FAILED_LINE).toBe("\r\n\x1b[2mMarshell's core stopped and could not restart. Restart Marshell to continue.\x1b[0m\r\n");
  });
});
