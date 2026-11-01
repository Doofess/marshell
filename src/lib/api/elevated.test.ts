import { describe, expect, it, vi } from "vitest";
import type { PermissionDecision } from "../../generated/PermissionDecision";
import { decide, resize, sendInput, trustedSink, typeText } from "./elevated";

const allow: PermissionDecision = { behavior: "allow", scope: "once", message: null };

describe("the window's trusted path", () => {
  it("elevated_rows_send_input_through_invoke_not_the_socket: input becomes the elevated_input command", async () => {
    const ask = vi.fn().mockResolvedValue(undefined);
    await sendInput("t1", "ls\r", ask);
    expect(ask).toHaveBeenCalledWith("elevated_input", { tabId: "t1", text: "ls\r" });
    await trustedSink("t2", ask).input("x");
    expect(ask).toHaveBeenLastCalledWith("elevated_input", { tabId: "t2", text: "x" });
  });

  it("resize_of_an_elevated_row_goes_through_invoke", async () => {
    const ask = vi.fn().mockResolvedValue(undefined);
    await resize("t1", 120, 30, ask);
    expect(ask).toHaveBeenCalledWith("elevated_resize", { tabId: "t1", cols: 120, rows: 30 });
    await trustedSink("t1", ask).resize(80, 24);
    expect(ask).toHaveBeenLastCalledWith("elevated_resize", { tabId: "t1", cols: 80, rows: 24 });
  });

  it("an_elevated_approval_answer_goes_through_invoke: decide becomes the elevated_decide command", async () => {
    const ask = vi.fn().mockResolvedValue(undefined);
    await decide("r1", allow, ask);
    expect(ask).toHaveBeenCalledWith("elevated_decide", { rid: "r1", decision: allow });
  });

  it("an_invoke_error_shows_the_core_copy: Tauri's string rejection becomes an Error with that sentence", async () => {
    const copy = "Not an administrator session.";
    await expect(sendInput("t1", "x", vi.fn().mockRejectedValue(copy))).rejects.toThrow(new Error(copy));
    await expect(resize("t1", 1, 1, vi.fn().mockRejectedValue(copy))).rejects.toThrow(new Error(copy));
    await expect(decide("r1", allow, vi.fn().mockRejectedValue(copy))).rejects.toThrow(new Error(copy));
  });

  it("types a prompt-line text through the window for an elevated row and through the API for any other", async () => {
    const ask = vi.fn().mockResolvedValue(undefined);
    const viaApi = vi.fn().mockResolvedValue(undefined);
    await typeText("t1", "fix it", true, viaApi, ask);
    expect(ask).toHaveBeenCalledWith("elevated_input", { tabId: "t1", text: "fix it" });
    expect(viaApi).not.toHaveBeenCalled();
    await typeText("t1", "fix it", false, viaApi, ask);
    expect(viaApi).toHaveBeenCalledTimes(1);
    expect(ask).toHaveBeenCalledTimes(1);
  });

  it("keeps the no-Enter rule for an elevated row", async () => {
    const ask = vi.fn();
    await expect(typeText("t1", "a\nb", true, vi.fn(), ask)).rejects.toThrow("The text can't contain a line break.");
    expect(ask).not.toHaveBeenCalled();
  });
});
