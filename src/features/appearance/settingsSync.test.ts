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
      return { appearance: teal, pause_warned: false, onboarded: false, status_off: [] };
    });
    await expect(sync.load()).rejects.toThrow();
    const dark = { ...teal, theme: "dark" as const };
    expect(await sync.save(dark)).toEqual(dark);
    expect(api.get).toHaveBeenCalledTimes(2);
    expect(api.put).toHaveBeenCalledTimes(1);
  });

  it("a save sends back the pause warning it read", async () => {
    const { api, sync } = setup(async () => ({ appearance: teal, pause_warned: true, onboarded: false, status_off: [] }));
    await sync.load();
    await sync.save(teal);
    expect(api.put).toHaveBeenCalledWith(ep, { appearance: teal, pause_warned: true, onboarded: false, status_off: [] });
  });

  it("after one successful read a save does not read again", async () => {
    const { api, sync } = setup(async () => ({ appearance: teal, pause_warned: true, onboarded: false, status_off: [] }));
    expect(await sync.load()).toEqual(teal);
    await sync.save(teal);
    expect(api.get).toHaveBeenCalledTimes(1);
    expect(api.put).toHaveBeenCalledTimes(1);
  });

  it("markPauseWarned sends the last appearance with pause_warned true, and the getter follows", async () => {
    const { api, sync } = setup(async () => ({ appearance: teal, pause_warned: false, onboarded: false, status_off: [] }));
    await sync.load();
    expect(sync.pauseWarned).toBe(false);
    await sync.markPauseWarned();
    expect(api.put).toHaveBeenCalledWith(ep, { appearance: teal, pause_warned: true, onboarded: false, status_off: [] });
    expect(sync.pauseWarned).toBe(true);
  });

  it("markPauseWarned reads first when nothing was read, and sends nothing if that fails", async () => {
    const { api, sync } = setup(async () => {
      throw new Error("no core");
    });
    await expect(sync.markPauseWarned()).rejects.toBeInstanceOf(SettingsReadError);
    expect(api.put).not.toHaveBeenCalled();
  });

  it("an appearance save after markPauseWarned keeps the warning", async () => {
    const { api, sync } = setup(async () => ({ appearance: teal, pause_warned: false, onboarded: false, status_off: [] }));
    await sync.markPauseWarned();
    await sync.save(teal);
    expect(api.put).toHaveBeenLastCalledWith(ep, { appearance: teal, pause_warned: true, onboarded: false, status_off: [] });
  });

  it("markOnboarded sends onboarded true and keeps pause_warned", async () => {
    const { api, sync } = setup(async () => ({ appearance: teal, pause_warned: true, onboarded: false, status_off: [] }));
    await sync.load();
    await sync.markOnboarded();
    expect(api.put).toHaveBeenCalledWith(ep, { appearance: teal, pause_warned: true, onboarded: true, status_off: [] });
  });

  it("markPauseWarned keeps onboarded", async () => {
    const { api, sync } = setup(async () => ({ appearance: teal, pause_warned: false, onboarded: true, status_off: [] }));
    await sync.load();
    await sync.markPauseWarned();
    expect(api.put).toHaveBeenCalledWith(ep, { appearance: teal, pause_warned: true, onboarded: true, status_off: [] });
  });
});

describe("SettingsSync: live status switch", () => {
  const read = (status_off: string[]) => async () => ({ appearance: teal, pause_warned: true, onboarded: true, status_off });
  it("turning it off adds the agent and keeps every other field", async () => {
    const { api, sync } = setup(read(["codex"]));
    await sync.setStatusOff("claude", true);
    expect(api.put).toHaveBeenCalledWith(ep, { appearance: teal, pause_warned: true, onboarded: true, status_off: ["codex", "claude"] });
    expect(sync.statusOff).toEqual(["codex", "claude"]);
  });
  it("turning it on removes the agent, and an unrelated save sends the list back", async () => {
    const { api, sync } = setup(read(["claude", "codex"]));
    await sync.setStatusOff("claude", false);
    expect(api.put).toHaveBeenLastCalledWith(ep, { appearance: teal, pause_warned: true, onboarded: true, status_off: ["codex"] });
    await sync.save(teal);
    expect(api.put).toHaveBeenLastCalledWith(ep, { appearance: teal, pause_warned: true, onboarded: true, status_off: ["codex"] });
  });
  it("sends nothing when the settings cannot be read", async () => {
    const { api, sync } = setup(async () => {
      throw new Error("no core");
    });
    await expect(sync.setStatusOff("claude", true)).rejects.toBeInstanceOf(SettingsReadError);
    expect(api.put).not.toHaveBeenCalled();
  });
});
