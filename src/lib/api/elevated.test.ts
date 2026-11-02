import { describe, expect, it, vi } from "vitest";
import type { PermissionDecision } from "../../generated/PermissionDecision";
import { ApiError } from "./client";
import { createSession, decide, reopenSession, restartSession, resize, resumeSession, sendInput, trustedSink, typeText } from "./elevated";

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

  const size = { cols: 100, rows: 30 };
  const started = { tab_id: "t9", number: 2 };

  it("creates an administrator session through elevated_create and any other through the API", async () => {
    const ask = vi.fn().mockResolvedValue(started);
    const viaApi = vi.fn().mockResolvedValue(started);
    const req = { cols: 80, rows: 24, elevated: true };
    expect(await createSession(req, viaApi, ask)).toEqual(started);
    expect(ask).toHaveBeenCalledWith("elevated_create", { req });
    expect(viaApi).not.toHaveBeenCalled();
    await createSession({ ...req, elevated: false }, viaApi, ask);
    expect(viaApi).toHaveBeenCalledTimes(1);
    expect(ask).toHaveBeenCalledTimes(1);
  });

  it("a refused administrator create rejects with an ApiError carrying the core sentence", async () => {
    const copy = "Administrator rights were declined. Nothing was started.";
    const err = await createSession({ cols: 1, rows: 1, elevated: true }, vi.fn(), vi.fn().mockRejectedValue(copy)).catch((e: unknown) => e);
    expect(err).toBeInstanceOf(ApiError);
    expect((err as ApiError).message).toBe(copy);
  });

  it("resumes and restarts an administrator row through the window, the core's refusal coming back as a reason", async () => {
    const ask = vi.fn().mockResolvedValue(started);
    const viaApi = vi.fn().mockResolvedValue({ ok: true });
    expect(await resumeSession(true, "t1", size, viaApi, ask)).toEqual({ ok: true });
    expect(ask).toHaveBeenLastCalledWith("elevated_resume", { tabId: "t1", ...size });
    expect(await restartSession(true, "t1", "bypass", size, viaApi, ask)).toEqual({ ok: true });
    expect(ask).toHaveBeenLastCalledWith("elevated_restart", { tabId: "t1", req: { mode: "bypass", ...size } });
    expect(viaApi).not.toHaveBeenCalled();
    const refused = vi.fn().mockRejectedValue("Administrator rights were declined. Nothing was started.");
    expect(await resumeSession(true, "t1", size, viaApi, refused)).toEqual({
      ok: false,
      reason: "Administrator rights were declined. Nothing was started.",
    });
  });

  it("resumes and restarts any other row through the API", async () => {
    const ask = vi.fn();
    const viaApi = vi.fn().mockResolvedValue({ ok: true });
    await resumeSession(false, "t1", size, viaApi, ask);
    await restartSession(false, "t1", "plan", size, viaApi, ask);
    expect(viaApi).toHaveBeenCalledTimes(2);
    expect(ask).not.toHaveBeenCalled();
  });

  it("reopens through the API, and through the window when the API says the row runs as administrator", async () => {
    const ask = vi.fn().mockResolvedValue(started);
    const ok = vi.fn().mockResolvedValue(started);
    await reopenSession("t1", ok, ask);
    expect(ok).toHaveBeenCalledTimes(1);
    expect(ask).not.toHaveBeenCalled();
    const refused = vi.fn().mockRejectedValue(new ApiError(409, "This session runs as administrator, so only Marshell's own window can start it."));
    await reopenSession("t1", refused, ask);
    expect(ask).toHaveBeenCalledWith("elevated_reopen", { tabId: "t1" });
    await reopenSession(undefined, refused, ask);
    expect(ask).toHaveBeenLastCalledWith("elevated_reopen", { tabId: null });
    // Any other failure stays the API's.
    const gone = vi.fn().mockRejectedValue(new ApiError(404, "Nothing to reopen."));
    await expect(reopenSession(undefined, gone, ask)).rejects.toThrow("Nothing to reopen.");
  });
});
