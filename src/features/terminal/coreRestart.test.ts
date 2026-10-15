import { describe, expect, it } from "vitest";
import { ENDED_LINE, FAILED_LINE } from "./coreRestart";

describe("core restart lines", () => {
  it("says the session ended and how to close it", () => {
    expect(ENDED_LINE).toBe("\r\n\x1b[2mSession ended. Close it with Ctrl+Shift+W.\x1b[0m\r\n");
  });

  it("says the core could not restart and what to do", () => {
    expect(FAILED_LINE).toBe("\r\n\x1b[2mMarshell's core stopped and could not restart. Restart Marshell to continue.\x1b[0m\r\n");
  });
});
