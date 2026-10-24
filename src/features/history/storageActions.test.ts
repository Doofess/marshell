import { describe, expect, it, vi } from "vitest";
import { ApiError } from "../../lib/api/client";
import { READ_FAILED, SettingsReadError } from "../appearance/settingsSync";
import { chooseBackupFolder, proposeLimits, storageErrorText } from "./storageActions";

const limits = { limit_gb: 5, limit_days: null };

describe("proposeLimits", () => {
  it("saves at once when the limits would remove nothing", async () => {
    const save = vi.fn(async () => {});
    const out = await proposeLimits(async () => ({ files: 0, bytes: 0, oldest_ms: null }), save, limits);
    expect(out).toBeNull();
    expect(save).toHaveBeenCalledWith(limits);
  });
  it("saves nothing and returns the question when files would go", async () => {
    const save = vi.fn(async () => {});
    const preview = { files: 3, bytes: 10, oldest_ms: 5 };
    expect(await proposeLimits(async () => preview, save, limits)).toEqual({ limits, preview });
    expect(save).not.toHaveBeenCalled();
  });
  it("lets a failed preview reject, saving nothing", async () => {
    const save = vi.fn(async () => {});
    await expect(proposeLimits(async () => Promise.reject(new Error("x")), save, limits)).rejects.toThrow("x");
    expect(save).not.toHaveBeenCalled();
  });
});

describe("chooseBackupFolder", () => {
  it("opens the picker at the current folder and saves the choice", async () => {
    const pick = vi.fn(async () => "E:/B");
    const save = vi.fn(async () => {});
    expect(await chooseBackupFolder(pick, save, "D:/Old")).toBe(true);
    expect(pick).toHaveBeenCalledWith("D:/Old");
    expect(save).toHaveBeenCalledWith("E:/B");
  });
  it("saves nothing when the picker is cancelled", async () => {
    const save = vi.fn(async () => {});
    expect(await chooseBackupFolder(async () => null, save, null)).toBe(false);
    expect(save).not.toHaveBeenCalled();
  });
});

describe("storageErrorText", () => {
  it("shows a refusal's body as it came", () => {
    expect(storageErrorText(new ApiError(400, "That folder doesn't exist."))).toBe("That folder doesn't exist.");
  });
  it("shows the unread-settings line, and a plain line for anything else", () => {
    expect(storageErrorText(new SettingsReadError())).toBe(READ_FAILED);
    expect(storageErrorText(new TypeError("fetch failed"))).not.toContain("fetch failed");
  });
});
