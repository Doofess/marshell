import { describe, expect, it, vi } from "vitest";
import type { Endpoint } from "../../generated/Endpoint";
import type { Settings } from "../../generated/Settings";
import { DEFAULT_APPEARANCE } from "./rootState";
import { READ_FAILED, SettingsReadError, SettingsSync } from "./settingsSync";

const ep: Endpoint = { port: 1, token: "t", platform: "windows" };
const teal = { ...DEFAULT_APPEARANCE, accent: "teal" };

function setup(get: () => Promise<Settings>) {
  const api = { get: vi.fn(get), put: vi.fn(async (_ep: Endpoint, s: Settings) => s) };
  return { api, sync: new SettingsSync(api, async () => ep) };
}

describe("SettingsSync", () => {
  it("a save before a successful read sends no PUT when the read still fails", async () => {
    const { api, sync } = setup(async () => {
      throw new Error("no core");
    });
    await expect(sync.save({ ...DEFAULT_APPEARANCE, theme: "dark" })).rejects.toBeInstanceOf(SettingsReadError);
    await expect(sync.save(DEFAULT_APPEARANCE)).rejects.toThrow(READ_FAILED);
    expect(api.put).not.toHaveBeenCalled();
  });

  it("a save retries the read first, then saves", async () => {
    let calls = 0;
    const { api, sync } = setup(async () => {
      if (++calls === 1) throw new Error("not yet");
      return { appearance: teal };
    });
    await expect(sync.load()).rejects.toThrow();
    const dark = { ...teal, theme: "dark" as const };
    expect(await sync.save(dark)).toEqual(dark);
    expect(api.get).toHaveBeenCalledTimes(2);
    expect(api.put).toHaveBeenCalledTimes(1);
  });

  it("after one successful read a save does not read again", async () => {
    const { api, sync } = setup(async () => ({ appearance: teal }));
    expect(await sync.load()).toEqual(teal);
    await sync.save(teal);
    expect(api.get).toHaveBeenCalledTimes(1);
    expect(api.put).toHaveBeenCalledTimes(1);
  });
});
