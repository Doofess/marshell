import { describe, expect, it, vi } from "vitest";
import type { Endpoint } from "../../generated/Endpoint";
import type { Settings } from "../../generated/Settings";
import { DEFAULT_APPEARANCE } from "./rootState";
import { READ_FAILED, SettingsReadError, SettingsSync } from "./settingsSync";

const ep: Endpoint = { port: 1, token: "t", platform: "windows" };
const teal = { ...DEFAULT_APPEARANCE, accent: "teal" };
const N = { needs_you: { banner: true, sound: true }, done: { banner: true, sound: true }, error: { banner: true, sound: true }, subagent_sound: false, quiet_hours: { enabled: false, start: "22:00", end: "08:00" }, terminal_alerts: true };
const N_OFF = { ...N, needs_you: { banner: true, sound: false } };

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
      return { appearance: teal, pause_warned: false, onboarded: false, close_to_tray: null, status_off: [], notifications: N, history_import: true, history_limit_gb: null, history_limit_days: null, history_backup_dir: null };
    });
    await expect(sync.load()).rejects.toThrow();
    const dark = { ...teal, theme: "dark" as const };
    expect(await sync.save(dark)).toEqual(dark);
    expect(api.get).toHaveBeenCalledTimes(2);
    expect(api.put).toHaveBeenCalledTimes(1);
  });

  it("a save sends back the pause warning it read", async () => {
    const { api, sync } = setup(async () => ({ appearance: teal, pause_warned: true, onboarded: false, close_to_tray: null, status_off: [], notifications: N, history_import: true, history_limit_gb: null, history_limit_days: null, history_backup_dir: null }));
    await sync.load();
    await sync.save(teal);
    expect(api.put).toHaveBeenCalledWith(ep, { appearance: teal, pause_warned: true, onboarded: false, close_to_tray: null, status_off: [], notifications: N, history_import: true, history_limit_gb: null, history_limit_days: null, history_backup_dir: null });
  });

  it("after one successful read a save does not read again", async () => {
    const { api, sync } = setup(async () => ({ appearance: teal, pause_warned: true, onboarded: false, close_to_tray: null, status_off: [], notifications: N, history_import: true, history_limit_gb: null, history_limit_days: null, history_backup_dir: null }));
    expect(await sync.load()).toEqual(teal);
    await sync.save(teal);
    expect(api.get).toHaveBeenCalledTimes(1);
    expect(api.put).toHaveBeenCalledTimes(1);
  });

  it("markPauseWarned sends the last appearance with pause_warned true, and the getter follows", async () => {
    const { api, sync } = setup(async () => ({ appearance: teal, pause_warned: false, onboarded: false, close_to_tray: null, status_off: [], notifications: N, history_import: true, history_limit_gb: null, history_limit_days: null, history_backup_dir: null }));
    await sync.load();
    expect(sync.pauseWarned).toBe(false);
    await sync.markPauseWarned();
    expect(api.put).toHaveBeenCalledWith(ep, { appearance: teal, pause_warned: true, onboarded: false, close_to_tray: null, status_off: [], notifications: N, history_import: true, history_limit_gb: null, history_limit_days: null, history_backup_dir: null });
    expect(sync.pauseWarned).toBe(true);
  });

  it("setCloseToTray saves the choice and the getter follows; every other save sends it back unchanged", async () => {
    const { api, sync } = setup(async () => ({ appearance: teal, pause_warned: false, onboarded: false, close_to_tray: null, status_off: [], notifications: N, history_import: true, history_limit_gb: null, history_limit_days: null, history_backup_dir: null }));
    await sync.load();
    expect(sync.closeToTray).toBeNull();
    await sync.setCloseToTray(false);
    expect(api.put).toHaveBeenLastCalledWith(ep, { appearance: teal, pause_warned: false, onboarded: false, close_to_tray: false, status_off: [], notifications: N, history_import: true, history_limit_gb: null, history_limit_days: null, history_backup_dir: null });
    expect(sync.closeToTray).toBe(false);
    await sync.markOnboarded();
    expect(api.put).toHaveBeenLastCalledWith(ep, expect.objectContaining({ close_to_tray: false, onboarded: true }));
    await sync.save(teal);
    expect(api.put).toHaveBeenLastCalledWith(ep, expect.objectContaining({ close_to_tray: false }));
  });

  it("setHistoryImport saves the switch, and a later appearance save sends it back unchanged", async () => {
    const { api, sync } = setup(async () => ({ appearance: teal, pause_warned: false, onboarded: false, close_to_tray: null, status_off: [], notifications: N, history_import: true, history_limit_gb: null, history_limit_days: null, history_backup_dir: null }));
    await sync.load();
    expect(sync.historyImport).toBe(true);
    await sync.setHistoryImport(false);
    expect(api.put).toHaveBeenLastCalledWith(ep, { appearance: teal, pause_warned: false, onboarded: false, close_to_tray: null, status_off: [], notifications: N, history_import: false, history_limit_gb: null, history_limit_days: null, history_backup_dir: null });
    expect(sync.historyImport).toBe(false);
    await sync.save(teal);
    expect(api.put).toHaveBeenLastCalledWith(ep, expect.objectContaining({ history_import: false, history_limit_gb: null, history_limit_days: null, history_backup_dir: null }));
  });

  it("every save sends back the retention limits it read", async () => {
    const { api, sync } = setup(async () => ({ appearance: teal, pause_warned: false, onboarded: false, close_to_tray: null, status_off: [], notifications: N, history_import: true, history_limit_gb: 5, history_limit_days: 90, history_backup_dir: null }));
    await sync.load();
    await sync.save(teal);
    expect(api.put).toHaveBeenLastCalledWith(ep, expect.objectContaining({ history_limit_gb: 5, history_limit_days: 90, history_backup_dir: null }));
    await sync.markOnboarded();
    expect(api.put).toHaveBeenLastCalledWith(ep, expect.objectContaining({ history_limit_gb: 5, history_limit_days: 90, history_backup_dir: null }));
  });

  it("every save sends back the backup folder it read", async () => {
    const { api, sync } = setup(async () => ({ appearance: teal, pause_warned: false, onboarded: false, close_to_tray: null, status_off: [], notifications: N, history_import: true, history_limit_gb: null, history_limit_days: null, history_backup_dir: "D:\Backups" }));
    await sync.load();
    await sync.save(teal);
    expect(api.put).toHaveBeenLastCalledWith(ep, expect.objectContaining({ history_backup_dir: "D:\Backups" }));
    await sync.markOnboarded();
    expect(api.put).toHaveBeenLastCalledWith(ep, expect.objectContaining({ history_backup_dir: "D:\Backups" }));
  });

  it("setHistoryLimits saves both limits, and a later appearance save sends them back unchanged", async () => {
    const { api, sync } = setup(async () => ({ appearance: teal, pause_warned: false, onboarded: false, close_to_tray: null, status_off: [], notifications: N, history_import: true, history_limit_gb: null, history_limit_days: null, history_backup_dir: "D:/B" }));
    await sync.load();
    expect(sync.historyLimits).toEqual({ limit_gb: null, limit_days: null });
    await sync.setHistoryLimits({ limit_gb: 5, limit_days: 90 });
    expect(api.put).toHaveBeenLastCalledWith(ep, { appearance: teal, pause_warned: false, onboarded: false, close_to_tray: null, status_off: [], notifications: N, history_import: true, history_limit_gb: 5, history_limit_days: 90, history_backup_dir: "D:/B" });
    expect(sync.historyLimits).toEqual({ limit_gb: 5, limit_days: 90 });
    await sync.save(teal);
    expect(api.put).toHaveBeenLastCalledWith(ep, expect.objectContaining({ history_limit_gb: 5, history_limit_days: 90, history_backup_dir: "D:/B" }));
  });

  it("setHistoryBackupDir saves the folder or clears it, keeping the limits", async () => {
    const { api, sync } = setup(async () => ({ appearance: teal, pause_warned: false, onboarded: false, close_to_tray: null, status_off: [], notifications: N, history_import: true, history_limit_gb: 2, history_limit_days: null, history_backup_dir: null }));
    await sync.load();
    expect(sync.historyBackupDir).toBeNull();
    await sync.setHistoryBackupDir("E:/Backups");
    expect(api.put).toHaveBeenLastCalledWith(ep, expect.objectContaining({ history_backup_dir: "E:/Backups", history_limit_gb: 2 }));
    expect(sync.historyBackupDir).toBe("E:/Backups");
    await sync.setHistoryBackupDir(null);
    expect(api.put).toHaveBeenLastCalledWith(ep, expect.objectContaining({ history_backup_dir: null, history_limit_gb: 2 }));
    expect(sync.historyBackupDir).toBeNull();
  });

  it("a refused history save rejects with the core's error and keeps the values it had", async () => {
    const { api, sync } = setup(async () => ({ appearance: teal, pause_warned: false, onboarded: false, close_to_tray: null, status_off: [], notifications: N, history_import: true, history_limit_gb: null, history_limit_days: null, history_backup_dir: null }));
    await sync.load();
    api.put.mockRejectedValueOnce(new Error("That folder doesn't exist."));
    await expect(sync.setHistoryBackupDir("Z:/nope")).rejects.toThrow("That folder doesn't exist.");
    expect(sync.historyBackupDir).toBeNull();
  });

  it("setHistoryLimits sends nothing when the read fails", async () => {
    const { api, sync } = setup(async () => {
      throw new Error("no core");
    });
    await expect(sync.setHistoryLimits({ limit_gb: 1, limit_days: null })).rejects.toBeInstanceOf(SettingsReadError);
    expect(api.put).not.toHaveBeenCalled();
  });

  it("setHistoryImport sends nothing when the read fails", async () => {
    const { api, sync } = setup(async () => {
      throw new Error("no core");
    });
    await expect(sync.setHistoryImport(false)).rejects.toBeInstanceOf(SettingsReadError);
    expect(api.put).not.toHaveBeenCalled();
  });

  it("setCloseToTray reads first when nothing was read, and sends nothing if that fails", async () => {
    const { api, sync } = setup(async () => {
      throw new Error("no core");
    });
    await expect(sync.setCloseToTray(true)).rejects.toBeInstanceOf(SettingsReadError);
    expect(api.put).not.toHaveBeenCalled();
  });

  it("markPauseWarned reads first when nothing was read, and sends nothing if that fails", async () => {
    const { api, sync } = setup(async () => {
      throw new Error("no core");
    });
    await expect(sync.markPauseWarned()).rejects.toBeInstanceOf(SettingsReadError);
    expect(api.put).not.toHaveBeenCalled();
  });

  it("an appearance save after markPauseWarned keeps the warning", async () => {
    const { api, sync } = setup(async () => ({ appearance: teal, pause_warned: false, onboarded: false, close_to_tray: null, status_off: [], notifications: N, history_import: true, history_limit_gb: null, history_limit_days: null, history_backup_dir: null }));
    await sync.markPauseWarned();
    await sync.save(teal);
    expect(api.put).toHaveBeenLastCalledWith(ep, { appearance: teal, pause_warned: true, onboarded: false, close_to_tray: null, status_off: [], notifications: N, history_import: true, history_limit_gb: null, history_limit_days: null, history_backup_dir: null });
  });

  it("markOnboarded sends onboarded true and keeps pause_warned", async () => {
    const { api, sync } = setup(async () => ({ appearance: teal, pause_warned: true, onboarded: false, close_to_tray: null, status_off: [], notifications: N, history_import: true, history_limit_gb: null, history_limit_days: null, history_backup_dir: null }));
    await sync.load();
    await sync.markOnboarded();
    expect(api.put).toHaveBeenCalledWith(ep, { appearance: teal, pause_warned: true, onboarded: true, close_to_tray: null, status_off: [], notifications: N, history_import: true, history_limit_gb: null, history_limit_days: null, history_backup_dir: null });
  });

  it("markPauseWarned keeps onboarded", async () => {
    const { api, sync } = setup(async () => ({ appearance: teal, pause_warned: false, onboarded: true, close_to_tray: null, status_off: [], notifications: N, history_import: true, history_limit_gb: null, history_limit_days: null, history_backup_dir: null }));
    await sync.load();
    await sync.markPauseWarned();
    expect(api.put).toHaveBeenCalledWith(ep, { appearance: teal, pause_warned: true, onboarded: true, close_to_tray: null, status_off: [], notifications: N, history_import: true, history_limit_gb: null, history_limit_days: null, history_backup_dir: null });
  });
});

describe("SettingsSync: live status switch", () => {
  const read = (status_off: string[]) => async () => ({ appearance: teal, pause_warned: true, onboarded: true, close_to_tray: null, status_off, notifications: N, history_import: true, history_limit_gb: null, history_limit_days: null, history_backup_dir: null });
  it("turning it off adds the agent and keeps every other field", async () => {
    const { api, sync } = setup(read(["codex"]));
    await sync.setStatusOff("claude", true);
    expect(api.put).toHaveBeenCalledWith(ep, { appearance: teal, pause_warned: true, onboarded: true, close_to_tray: null, status_off: ["codex", "claude"], notifications: N, history_import: true, history_limit_gb: null, history_limit_days: null, history_backup_dir: null });
    expect(sync.statusOff).toEqual(["codex", "claude"]);
  });
  it("turning it on removes the agent, and an unrelated save sends the list back", async () => {
    const { api, sync } = setup(read(["claude", "codex"]));
    await sync.setStatusOff("claude", false);
    expect(api.put).toHaveBeenLastCalledWith(ep, { appearance: teal, pause_warned: true, onboarded: true, close_to_tray: null, status_off: ["codex"], notifications: N, history_import: true, history_limit_gb: null, history_limit_days: null, history_backup_dir: null });
    await sync.save(teal);
    expect(api.put).toHaveBeenLastCalledWith(ep, { appearance: teal, pause_warned: true, onboarded: true, close_to_tray: null, status_off: ["codex"], notifications: N, history_import: true, history_limit_gb: null, history_limit_days: null, history_backup_dir: null });
  });
  it("sends nothing when the settings cannot be read", async () => {
    const { api, sync } = setup(async () => {
      throw new Error("no core");
    });
    await expect(sync.setStatusOff("claude", true)).rejects.toBeInstanceOf(SettingsReadError);
    expect(api.put).not.toHaveBeenCalled();
  });
});

describe("SettingsSync: notifications", () => {
  const read = (n: typeof N) => async () => ({ appearance: teal, pause_warned: true, onboarded: true, close_to_tray: null, status_off: ["codex"], notifications: n, history_import: true, history_limit_gb: null, history_limit_days: null, history_backup_dir: null });
  it("saving them keeps every other field, and other saves send them back", async () => {
    const { api, sync } = setup(read(N));
    await sync.setNotifications(N_OFF);
    expect(api.put).toHaveBeenLastCalledWith(ep, { appearance: teal, pause_warned: true, onboarded: true, close_to_tray: null, status_off: ["codex"], notifications: N_OFF, history_import: true, history_limit_gb: null, history_limit_days: null, history_backup_dir: null });
    expect(sync.notifications).toEqual(N_OFF);
    await sync.setStatusOff("claude", true);
    expect(api.put).toHaveBeenLastCalledWith(ep, { appearance: teal, pause_warned: true, onboarded: true, close_to_tray: null, status_off: ["codex", "claude"], notifications: N_OFF, history_import: true, history_limit_gb: null, history_limit_days: null, history_backup_dir: null });
  });
  it("an appearance save sends the stored settings back unchanged", async () => {
    const { api, sync } = setup(read(N_OFF));
    await sync.save(teal);
    expect(api.put).toHaveBeenLastCalledWith(ep, { appearance: teal, pause_warned: true, onboarded: true, close_to_tray: null, status_off: ["codex"], notifications: N_OFF, history_import: true, history_limit_gb: null, history_limit_days: null, history_backup_dir: null });
  });
});
