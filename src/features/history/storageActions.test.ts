import { describe, expect, it, vi } from "vitest";
import { ApiError } from "../../lib/api/client";
import { READ_FAILED, SettingsReadError } from "../appearance/settingsSync";
import type { StorageReport } from "../../generated/StorageReport";
import { chooseBackupFolder, proposeLimits, removalQuestion, reportAfterCleanup, storageErrorText } from "./storageActions";

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
  it("announces the question it returns, and announces nothing when it saves at once", async () => {
    const announce = vi.fn();
    const preview = { files: 2, bytes: 10, oldest_ms: null };
    const ask = await proposeLimits(async () => preview, async () => {}, limits, announce);
    expect(announce).toHaveBeenCalledTimes(1);
    expect(announce).toHaveBeenCalledWith(removalQuestion({ limits, preview }));
    expect(announce.mock.calls[0]?.[0]).toContain("This removes 2 recordings and images now");
    expect(ask).toEqual({ limits, preview });
    const quiet = vi.fn();
    expect(await proposeLimits(async () => ({ files: 0, bytes: 0, oldest_ms: null }), async () => {}, limits, quiet)).toBeNull();
    expect(quiet).not.toHaveBeenCalled();
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

describe("reportAfterCleanup", () => {
  const at = (ms: number | null) => ({ last_cleanup: ms === null ? null : { at_ms: ms, files: 1, bytes: 1 } }) as StorageReport;
  const noWait = async () => {};

  it("reads again until the report holds a cleanup newer than the one before the save", async () => {
    const get = vi.fn().mockResolvedValueOnce(at(100)).mockResolvedValueOnce(at(100)).mockResolvedValueOnce(at(200));
    expect((await reportAfterCleanup(get, 100, noWait)).last_cleanup?.at_ms).toBe(200);
    expect(get).toHaveBeenCalledTimes(3);
  });

  it("takes a first cleanup when there was none before", async () => {
    const get = vi.fn().mockResolvedValueOnce(at(null)).mockResolvedValueOnce(at(5));
    expect((await reportAfterCleanup(get, null, noWait)).last_cleanup?.at_ms).toBe(5);
  });

  it("gives up after a few tries and returns the last report", async () => {
    const get = vi.fn().mockResolvedValue(at(100));
    expect((await reportAfterCleanup(get, 100, noWait)).last_cleanup?.at_ms).toBe(100);
    expect(get).toHaveBeenCalledTimes(20);
  });
});
