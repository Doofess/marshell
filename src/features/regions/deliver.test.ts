import { describe, expect, it } from "vitest";
import { FIXTURES } from "../sidebar/fixtures";
import { clipboardText, deliveryFor, noticeFor } from "./deliver";

const agentRow = (name: string) => ({ ...FIXTURES.find((f) => f.id === "working")!, name });
const shellRow = (name: string) => ({ ...FIXTURES.find((f) => f.id === "tool-powershell")!, name });

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
});
