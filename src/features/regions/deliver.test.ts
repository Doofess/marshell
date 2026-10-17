import { describe, expect, it } from "vitest";
import { ApiError } from "../../lib/api/client";
import { FIXTURES } from "../sidebar/fixtures";
import { addFailure, clipboardText, deliveryFor, noticeFor } from "./deliver";

const agentRow = (name: string) => ({ ...FIXTURES.find((f) => f.id === "working")!, name });
const shellRow = (name: string) => ({ ...FIXTURES.find((f) => f.id === "tool-powershell")!, name });
const rowWithStatus = (status: "paused" | "ended", name: string) => ({ ...FIXTURES.find((f) => f.id === "working")!, status, name });

describe("region delivery", () => {
  it("pastes into an agent and copies for a plain terminal", () => {
    expect(deliveryFor(null)).toBeNull();
    expect(deliveryFor(agentRow("billing"))).toEqual({ name: "billing", via: "paste" });
    expect(deliveryFor(shellRow("pwsh 2"))).toEqual({ name: "pwsh 2", via: "clipboard" });
  });
  it("builds the clipboard text and the notices", () => {
    expect(clipboardText('"C:\\a\\1.png"', "chart.png: here")).toBe('"C:\\a\\1.png" chart.png: here');
    expect(noticeFor("paste", "billing")).toBe("Added the region to billing.");
    expect(noticeFor("clipboard", "pwsh 2")).toBe("pwsh 2 has no prompt line, so the region is on the clipboard.");
  });
  it("refuses a paused session before anything is saved", () => {
    expect(deliveryFor(rowWithStatus("paused", "migration"))).toEqual({ name: "migration", refused: "migration is paused." });
  });
  it("refuses an ended session", () => {
    expect(deliveryFor(rowWithStatus("ended", "billing"))).toEqual({ name: "billing", refused: "billing has ended." });
  });
});

describe("addFailure", () => {
  it("reads the reason out of the core's JSON body", () => {
    expect(addFailure(new ApiError(409, '{"reason":"It is paused."}'))).toBe("It is paused.");
  });
  it("gives a 404 with no body a plain reason", () => {
    expect(addFailure(new ApiError(404, ""))).toBe("The session has ended.");
  });
  it("keeps a plain-text body, and names the status when there is no body", () => {
    expect(addFailure(new ApiError(400, "That isn't a saved attachment."))).toBe("That isn't a saved attachment.");
    expect(addFailure(new ApiError(500, ""))).toBe("The core refused it (500).");
    expect(addFailure(new Error("network down"))).toBe("network down");
  });
});
