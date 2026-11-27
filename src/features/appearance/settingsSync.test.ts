import { describe, expect, it, vi } from "vitest";
import type { Endpoint } from "../../generated/Endpoint";
import type { Settings } from "../../generated/Settings";
import { DEFAULT_APPEARANCE } from "./rootState";
import { READ_FAILED, SettingsReadError, SettingsSync, loadWithRetry } from "./settingsSync";

const ep: Endpoint = { port: 1, token: "t", platform: "windows" };
const large = { ...DEFAULT_APPEARANCE, text_size: "large" as const };
const N = { needs_you: { banner: true, sound: true }, done: { banner: true, sound: true }, error: { banner: true, sound: true }, subagent_sound: false, quiet_hours: { enabled: false, start: "22:00", end: "08:00" }, terminal_alerts: true };
const BR = { terminal_links: true, open_agent_html: true, cookies: "always" as const };
const FI = { preview_tabs: true, auto_save: false, tab_indents: true, word_wrap: true, indent_guides: true, whitespace: false, scroll_past_end: false, tab_size: 4, tree_position: "automatic" as const };
const CH = { hide_whitespace: false, wrap: false, pull_requests: true, resolved_comments: false };
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
      return { appearance: large, pause_warned: false, onboarded: false, close_to_tray: null, status_off: [], notifications: N, history_import: true, history_limit_gb: null, history_limit_days: null, history_backup_dir: null, default_folder: null, browser: BR, files: FI, changes: CH };
    });
    await expect(sync.load()).rejects.toThrow();
    const dark = { ...large, theme: "dark" as const };
    expect(await sync.save(dark)).toEqual(dark);
    expect(api.get).toHaveBeenCalledTimes(2);
    expect(api.put).toHaveBeenCalledTimes(1);
  });

  it("a save sends back the pause warning it read", async () => {
    const { api, sync } = setup(async () => ({ appearance: large, pause_warned: true, onboarded: false, close_to_tray: null, status_off: [], notifications: N, history_import: true, history_limit_gb: null, history_limit_days: null, history_backup_dir: null, default_folder: null, browser: BR, files: FI, changes: CH }));
    await sync.load();
    await sync.save(large);
    expect(api.put).toHaveBeenCalledWith(ep, { appearance: large, pause_warned: true, onboarded: false, close_to_tray: null, status_off: [], notifications: N, history_import: true, history_limit_gb: null, history_limit_days: null, history_backup_dir: null, default_folder: null, browser: BR, files: FI, changes: CH });
  });

  it("after one successful read a save does not read again", async () => {
    const { api, sync } = setup(async () => ({ appearance: large, pause_warned: true, onboarded: false, close_to_tray: null, status_off: [], notifications: N, history_import: true, history_limit_gb: null, history_limit_days: null, history_backup_dir: null, default_folder: null, browser: BR, files: FI, changes: CH }));
    expect(await sync.load()).toEqual(large);
    await sync.save(large);
    expect(api.get).toHaveBeenCalledTimes(1);
    expect(api.put).toHaveBeenCalledTimes(1);
  });

  it("markPauseWarned sends the last appearance with pause_warned true, and the getter follows", async () => {
    const { api, sync } = setup(async () => ({ appearance: large, pause_warned: false, onboarded: false, close_to_tray: null, status_off: [], notifications: N, history_import: true, history_limit_gb: null, history_limit_days: null, history_backup_dir: null, default_folder: null, browser: BR, files: FI, changes: CH }));
    await sync.load();
    expect(sync.pauseWarned).toBe(false);
    await sync.markPauseWarned();
    expect(api.put).toHaveBeenCalledWith(ep, { appearance: large, pause_warned: true, onboarded: false, close_to_tray: null, status_off: [], notifications: N, history_import: true, history_limit_gb: null, history_limit_days: null, history_backup_dir: null, default_folder: null, browser: BR, files: FI, changes: CH });
    expect(sync.pauseWarned).toBe(true);
  });

  it("setCloseToTray saves the choice and the getter follows; every other save sends it back unchanged", async () => {
    const { api, sync } = setup(async () => ({ appearance: large, pause_warned: false, onboarded: false, close_to_tray: null, status_off: [], notifications: N, history_import: true, history_limit_gb: null, history_limit_days: null, history_backup_dir: null, default_folder: null, browser: BR, files: FI, changes: CH }));
    await sync.load();
    expect(sync.closeToTray).toBeNull();
    await sync.setCloseToTray(false);
    expect(api.put).toHaveBeenLastCalledWith(ep, { appearance: large, pause_warned: false, onboarded: false, close_to_tray: false, status_off: [], notifications: N, history_import: true, history_limit_gb: null, history_limit_days: null, history_backup_dir: null, default_folder: null, browser: BR, files: FI, changes: CH });
    expect(sync.closeToTray).toBe(false);
    await sync.markOnboarded();
    expect(api.put).toHaveBeenLastCalledWith(ep, expect.objectContaining({ close_to_tray: false, onboarded: true }));
    await sync.save(large);
    expect(api.put).toHaveBeenLastCalledWith(ep, expect.objectContaining({ close_to_tray: false }));
  });

  it("setHistoryImport saves the switch, and a later appearance save sends it back unchanged", async () => {
    const { api, sync } = setup(async () => ({ appearance: large, pause_warned: false, onboarded: false, close_to_tray: null, status_off: [], notifications: N, history_import: true, history_limit_gb: null, history_limit_days: null, history_backup_dir: null, default_folder: null, browser: BR, files: FI, changes: CH }));
    await sync.load();
    expect(sync.historyImport).toBe(true);
    await sync.setHistoryImport(false);
    expect(api.put).toHaveBeenLastCalledWith(ep, { appearance: large, pause_warned: false, onboarded: false, close_to_tray: null, status_off: [], notifications: N, history_import: false, history_limit_gb: null, history_limit_days: null, history_backup_dir: null, default_folder: null, browser: BR, files: FI, changes: CH });
    expect(sync.historyImport).toBe(false);
    await sync.save(large);
    expect(api.put).toHaveBeenLastCalledWith(ep, expect.objectContaining({ history_import: false, history_limit_gb: null, history_limit_days: null, history_backup_dir: null, default_folder: null, browser: BR, files: FI, changes: CH }));
  });

  it("every save sends back the retention limits it read", async () => {
    const { api, sync } = setup(async () => ({ appearance: large, pause_warned: false, onboarded: false, close_to_tray: null, status_off: [], notifications: N, history_import: true, history_limit_gb: 5, history_limit_days: 90, history_backup_dir: null, default_folder: null, browser: BR, files: FI, changes: CH }));
    await sync.load();
    await sync.save(large);
    expect(api.put).toHaveBeenLastCalledWith(ep, expect.objectContaining({ history_limit_gb: 5, history_limit_days: 90, history_backup_dir: null }));
    await sync.markOnboarded();
    expect(api.put).toHaveBeenLastCalledWith(ep, expect.objectContaining({ history_limit_gb: 5, history_limit_days: 90, history_backup_dir: null }));
  });

  it("every save sends back the backup folder it read", async () => {
    const { api, sync } = setup(async () => ({ appearance: large, pause_warned: false, onboarded: false, close_to_tray: null, status_off: [], notifications: N, history_import: true, history_limit_gb: null, history_limit_days: null, history_backup_dir: "D:\Backups", default_folder: null, browser: BR, files: FI, changes: CH }));
    await sync.load();
    await sync.save(large);
    expect(api.put).toHaveBeenLastCalledWith(ep, expect.objectContaining({ history_backup_dir: "D:\Backups" }));
    await sync.markOnboarded();
    expect(api.put).toHaveBeenLastCalledWith(ep, expect.objectContaining({ history_backup_dir: "D:\Backups" }));
  });

  it("a theme change keeps the default folder", async () => {
    const { api, sync } = setup(async () => ({ appearance: large, pause_warned: false, onboarded: false, close_to_tray: null, status_off: [], notifications: N, history_import: true, history_limit_gb: null, history_limit_days: null, history_backup_dir: null, default_folder: "D:/Work", browser: BR, files: FI, changes: CH }));
    await sync.load();
    await sync.save(large);
    expect(api.put).toHaveBeenLastCalledWith(ep, expect.objectContaining({ default_folder: "D:/Work" }));
    await sync.setCloseToTray(true);
    expect(api.put).toHaveBeenLastCalledWith(ep, expect.objectContaining({ default_folder: "D:/Work", close_to_tray: true }));
  });

  it("setHistoryLimits saves both limits, and a later appearance save sends them back unchanged", async () => {
    const { api, sync } = setup(async () => ({ appearance: large, pause_warned: false, onboarded: false, close_to_tray: null, status_off: [], notifications: N, history_import: true, history_limit_gb: null, history_limit_days: null, history_backup_dir: "D:/B", default_folder: null, browser: BR, files: FI, changes: CH }));
    await sync.load();
    expect(sync.historyLimits).toEqual({ limit_gb: null, limit_days: null });
    await sync.setHistoryLimits({ limit_gb: 5, limit_days: 90 });
    expect(api.put).toHaveBeenLastCalledWith(ep, { appearance: large, pause_warned: false, onboarded: false, close_to_tray: null, status_off: [], notifications: N, history_import: true, history_limit_gb: 5, history_limit_days: 90, history_backup_dir: "D:/B", default_folder: null, browser: BR, files: FI, changes: CH });
    expect(sync.historyLimits).toEqual({ limit_gb: 5, limit_days: 90 });
    await sync.save(large);
    expect(api.put).toHaveBeenLastCalledWith(ep, expect.objectContaining({ history_limit_gb: 5, history_limit_days: 90, history_backup_dir: "D:/B" }));
  });

  it("setHistoryBackupDir saves the folder or clears it, keeping the limits", async () => {
    const { api, sync } = setup(async () => ({ appearance: large, pause_warned: false, onboarded: false, close_to_tray: null, status_off: [], notifications: N, history_import: true, history_limit_gb: 2, history_limit_days: null, history_backup_dir: null, default_folder: null, browser: BR, files: FI, changes: CH }));
    await sync.load();
    expect(sync.historyBackupDir).toBeNull();
    await sync.setHistoryBackupDir("E:/Backups");
    expect(api.put).toHaveBeenLastCalledWith(ep, expect.objectContaining({ history_backup_dir: "E:/Backups", history_limit_gb: 2 }));
    expect(sync.historyBackupDir).toBe("E:/Backups");
    await sync.setHistoryBackupDir(null);
    expect(api.put).toHaveBeenLastCalledWith(ep, expect.objectContaining({ history_backup_dir: null, history_limit_gb: 2 }));
    expect(sync.historyBackupDir).toBeNull();
  });

  it("setDefaultFolder saves the folder and keeps the other settings", async () => {
    const { api, sync } = setup(async () => ({ appearance: large, pause_warned: false, onboarded: false, close_to_tray: null, status_off: [], notifications: N, history_import: true, history_limit_gb: 2, history_limit_days: null, history_backup_dir: "D:/B", default_folder: null, browser: BR, files: FI, changes: CH }));
    await sync.load();
    expect(sync.defaultFolder).toBeNull();
    await sync.setDefaultFolder("D:/Work");
    expect(api.put).toHaveBeenLastCalledWith(ep, expect.objectContaining({ default_folder: "D:/Work", history_limit_gb: 2, history_backup_dir: "D:/B" }));
    expect(sync.defaultFolder).toBe("D:/Work");
  });

  it("setDefaultFolder(null) clears it", async () => {
    const { api, sync } = setup(async () => ({ appearance: large, pause_warned: false, onboarded: false, close_to_tray: null, status_off: [], notifications: N, history_import: true, history_limit_gb: null, history_limit_days: null, history_backup_dir: null, default_folder: "D:/Work", browser: BR, files: FI, changes: CH }));
    await sync.load();
    expect(sync.defaultFolder).toBe("D:/Work");
    await sync.setDefaultFolder(null);
    expect(api.put).toHaveBeenLastCalledWith(ep, expect.objectContaining({ default_folder: null }));
    expect(sync.defaultFolder).toBeNull();
  });

  it("a refused history save rejects with the core's error and keeps the values it had", async () => {
    const { api, sync } = setup(async () => ({ appearance: large, pause_warned: false, onboarded: false, close_to_tray: null, status_off: [], notifications: N, history_import: true, history_limit_gb: null, history_limit_days: null, history_backup_dir: null, default_folder: null, browser: BR, files: FI, changes: CH }));
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
    const { api, sync } = setup(async () => ({ appearance: large, pause_warned: false, onboarded: false, close_to_tray: null, status_off: [], notifications: N, history_import: true, history_limit_gb: null, history_limit_days: null, history_backup_dir: null, default_folder: null, browser: BR, files: FI, changes: CH }));
    await sync.markPauseWarned();
    await sync.save(large);
    expect(api.put).toHaveBeenLastCalledWith(ep, { appearance: large, pause_warned: true, onboarded: false, close_to_tray: null, status_off: [], notifications: N, history_import: true, history_limit_gb: null, history_limit_days: null, history_backup_dir: null, default_folder: null, browser: BR, files: FI, changes: CH });
  });

  it("markOnboarded sends onboarded true and keeps pause_warned", async () => {
    const { api, sync } = setup(async () => ({ appearance: large, pause_warned: true, onboarded: false, close_to_tray: null, status_off: [], notifications: N, history_import: true, history_limit_gb: null, history_limit_days: null, history_backup_dir: null, default_folder: null, browser: BR, files: FI, changes: CH }));
    await sync.load();
    await sync.markOnboarded();
    expect(api.put).toHaveBeenCalledWith(ep, { appearance: large, pause_warned: true, onboarded: true, close_to_tray: null, status_off: [], notifications: N, history_import: true, history_limit_gb: null, history_limit_days: null, history_backup_dir: null, default_folder: null, browser: BR, files: FI, changes: CH });
  });

  it("markPauseWarned keeps onboarded", async () => {
    const { api, sync } = setup(async () => ({ appearance: large, pause_warned: false, onboarded: true, close_to_tray: null, status_off: [], notifications: N, history_import: true, history_limit_gb: null, history_limit_days: null, history_backup_dir: null, default_folder: null, browser: BR, files: FI, changes: CH }));
    await sync.load();
    await sync.markPauseWarned();
    expect(api.put).toHaveBeenCalledWith(ep, { appearance: large, pause_warned: true, onboarded: true, close_to_tray: null, status_off: [], notifications: N, history_import: true, history_limit_gb: null, history_limit_days: null, history_backup_dir: null, default_folder: null, browser: BR, files: FI, changes: CH });
  });
});

describe("SettingsSync: live status switch", () => {
  const read = (status_off: string[]) => async () => ({ appearance: large, pause_warned: true, onboarded: true, close_to_tray: null, status_off, notifications: N, history_import: true, history_limit_gb: null, history_limit_days: null, history_backup_dir: null, default_folder: null, browser: BR, files: FI, changes: CH });
  it("turning it off adds the agent and keeps every other field", async () => {
    const { api, sync } = setup(read(["codex"]));
    await sync.setStatusOff("claude", true);
    expect(api.put).toHaveBeenCalledWith(ep, { appearance: large, pause_warned: true, onboarded: true, close_to_tray: null, status_off: ["codex", "claude"], notifications: N, history_import: true, history_limit_gb: null, history_limit_days: null, history_backup_dir: null, default_folder: null, browser: BR, files: FI, changes: CH });
    expect(sync.statusOff).toEqual(["codex", "claude"]);
  });
  it("turning it on removes the agent, and an unrelated save sends the list back", async () => {
    const { api, sync } = setup(read(["claude", "codex"]));
    await sync.setStatusOff("claude", false);
    expect(api.put).toHaveBeenLastCalledWith(ep, { appearance: large, pause_warned: true, onboarded: true, close_to_tray: null, status_off: ["codex"], notifications: N, history_import: true, history_limit_gb: null, history_limit_days: null, history_backup_dir: null, default_folder: null, browser: BR, files: FI, changes: CH });
    await sync.save(large);
    expect(api.put).toHaveBeenLastCalledWith(ep, { appearance: large, pause_warned: true, onboarded: true, close_to_tray: null, status_off: ["codex"], notifications: N, history_import: true, history_limit_gb: null, history_limit_days: null, history_backup_dir: null, default_folder: null, browser: BR, files: FI, changes: CH });
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
  const read = (n: typeof N) => async () => ({ appearance: large, pause_warned: true, onboarded: true, close_to_tray: null, status_off: ["codex"], notifications: n, history_import: true, history_limit_gb: null, history_limit_days: null, history_backup_dir: null, default_folder: null, browser: BR, files: FI, changes: CH });
  it("saving them keeps every other field, and other saves send them back", async () => {
    const { api, sync } = setup(read(N));
    await sync.setNotifications(N_OFF);
    expect(api.put).toHaveBeenLastCalledWith(ep, { appearance: large, pause_warned: true, onboarded: true, close_to_tray: null, status_off: ["codex"], notifications: N_OFF, history_import: true, history_limit_gb: null, history_limit_days: null, history_backup_dir: null, default_folder: null, browser: BR, files: FI, changes: CH });
    expect(sync.notifications).toEqual(N_OFF);
    await sync.setStatusOff("claude", true);
    expect(api.put).toHaveBeenLastCalledWith(ep, { appearance: large, pause_warned: true, onboarded: true, close_to_tray: null, status_off: ["codex", "claude"], notifications: N_OFF, history_import: true, history_limit_gb: null, history_limit_days: null, history_backup_dir: null, default_folder: null, browser: BR, files: FI, changes: CH });
  });
  it("an appearance save sends the stored settings back unchanged", async () => {
    const { api, sync } = setup(read(N_OFF));
    await sync.save(large);
    expect(api.put).toHaveBeenLastCalledWith(ep, { appearance: large, pause_warned: true, onboarded: true, close_to_tray: null, status_off: ["codex"], notifications: N_OFF, history_import: true, history_limit_gb: null, history_limit_days: null, history_backup_dir: null, default_folder: null, browser: BR, files: FI, changes: CH });
  });
});

describe("SettingsSync: browser", () => {
  const stored = { terminal_links: false, open_agent_html: true, cookies: "never" as const };
  const read = async () => ({ appearance: large, pause_warned: true, onboarded: true, close_to_tray: null, status_off: ["codex"], notifications: N, history_import: true, history_limit_gb: null, history_limit_days: null, history_backup_dir: null, default_folder: null, browser: stored, files: FI, changes: CH });
  it("is null before the first read, then what the core holds", async () => {
    const { sync } = setup(read);
    expect(sync.browser).toBeNull();
    expect(sync.readFailed).toBe(false);
    await sync.load();
    expect(sync.browser).toEqual(stored);
  });
  it("writes one changed field and keeps the others", async () => {
    const { api, sync } = setup(read);
    await sync.setBrowser({ cookies: "until-quit" });
    expect(api.put).toHaveBeenLastCalledWith(ep, { appearance: large, pause_warned: true, onboarded: true, close_to_tray: null, status_off: ["codex"], notifications: N, history_import: true, history_limit_gb: null, history_limit_days: null, history_backup_dir: null, default_folder: null, browser: { terminal_links: false, open_agent_html: true, cookies: "until-quit" }, files: FI, changes: CH });
    expect(sync.browser).toEqual({ terminal_links: false, open_agent_html: true, cookies: "until-quit" });
  });
  it("keeps the same object between changes and tells subscribers after a write", async () => {
    const { sync } = setup(read);
    await sync.load();
    expect(sync.browser).toBe(sync.browser);
    const listener = vi.fn();
    const off = sync.subscribe(listener);
    await sync.setBrowser({ terminal_links: true });
    expect(listener).toHaveBeenCalledTimes(1);
    off();
    await sync.setBrowser({ terminal_links: false });
    expect(listener).toHaveBeenCalledTimes(1);
  });
  it("two quick changes go out one after the other, the second built from the first's result", async () => {
    const { api, sync } = setup(read);
    await sync.load();
    const releases: (() => void)[] = [];
    api.put.mockImplementation((_ep: Endpoint, s: Settings) => new Promise<Settings>((resolve) => releases.push(() => resolve(s))));
    const a = sync.setBrowser({ cookies: "until-quit" });
    const b = sync.setBrowser({ open_agent_html: false });
    await new Promise((r) => setTimeout(r, 0));
    expect(api.put).toHaveBeenCalledTimes(1);
    releases[0]!();
    await a;
    await new Promise((r) => setTimeout(r, 0));
    expect(api.put).toHaveBeenCalledTimes(2);
    releases[1]!();
    await b;
    expect(api.put.mock.calls[1]![1].browser).toEqual({ terminal_links: false, open_agent_html: false, cookies: "until-quit" });
    expect(sync.browser).toEqual({ terminal_links: false, open_agent_html: false, cookies: "until-quit" });
  });
  it.each([
    ["an appearance save", (sync: SettingsSync) => sync.save({ ...large, theme: "dark" as const })],
    ["a flag write", (sync: SettingsSync) => sync.setCloseToTray(true)],
  ])("%s that overlaps a browser change waits for it and sends its result", async (_name, other) => {
    const { api, sync } = setup(read);
    await sync.load();
    const releases: (() => void)[] = [];
    api.put.mockImplementation((_ep: Endpoint, s: Settings) => new Promise<Settings>((resolve) => releases.push(() => resolve(s))));
    const a = sync.setBrowser({ cookies: "until-quit" });
    const b = other(sync);
    await new Promise((r) => setTimeout(r, 0));
    expect(api.put).toHaveBeenCalledTimes(1);
    releases[0]!();
    await a;
    await new Promise((r) => setTimeout(r, 0));
    expect(api.put).toHaveBeenCalledTimes(2);
    expect(api.put.mock.calls[1]![1].browser.cookies).toBe("until-quit");
    releases[1]!();
    await b;
    expect(sync.browser?.cookies).toBe("until-quit");
  });
  it("a change that fails doesn't stop the next one", async () => {
    const { api, sync } = setup(read);
    await sync.load();
    api.put.mockRejectedValueOnce(new Error("down"));
    const a = sync.setBrowser({ cookies: "never" });
    const b = sync.setBrowser({ open_agent_html: false });
    await expect(a).rejects.toThrow("down");
    await b;
    expect(sync.browser).toEqual({ terminal_links: false, open_agent_html: false, cookies: "never" });
  });
  it("says a failed read", async () => {
    const { sync } = setup(async () => {
      throw new Error("no core");
    });
    const listener = vi.fn();
    sync.subscribe(listener);
    await expect(sync.load()).rejects.toThrow();
    expect(sync.readFailed).toBe(true);
    expect(sync.browser).toBeNull();
    expect(listener).toHaveBeenCalled();
  });
});

describe("SettingsSync: files", () => {
  const stored = { ...FI, word_wrap: false, tab_size: 2 };
  const read = async () => ({ appearance: large, pause_warned: true, onboarded: true, close_to_tray: null, status_off: ["codex"], notifications: N, history_import: true, history_limit_gb: null, history_limit_days: null, history_backup_dir: null, default_folder: null, browser: BR, files: stored, changes: CH });
  it("is null before the first read, then what the core holds", async () => {
    const { sync } = setup(read);
    expect(sync.files).toBeNull();
    await sync.load();
    expect(sync.files).toEqual(stored);
  });
  it("writes one changed field and keeps the others", async () => {
    const { api, sync } = setup(read);
    await sync.setFiles({ auto_save: true });
    expect(api.put).toHaveBeenLastCalledWith(ep, { appearance: large, pause_warned: true, onboarded: true, close_to_tray: null, status_off: ["codex"], notifications: N, history_import: true, history_limit_gb: null, history_limit_days: null, history_backup_dir: null, default_folder: null, browser: BR, files: { ...stored, auto_save: true }, changes: CH });
    expect(sync.files).toEqual({ ...stored, auto_save: true });
  });
  it("keeps the same object between changes and tells subscribers after a write", async () => {
    const { sync } = setup(read);
    await sync.load();
    expect(sync.files).toBe(sync.files);
    const listener = vi.fn();
    sync.subscribe(listener);
    await sync.setFiles({ whitespace: true });
    expect(listener).toHaveBeenCalledTimes(1);
  });
  it("says a failed read", async () => {
    const { sync } = setup(async () => {
      throw new Error("no core");
    });
    await expect(sync.load()).rejects.toThrow();
    expect(sync.readFailed).toBe(true);
    expect(sync.files).toBeNull();
  });
});

describe("SettingsSync: changes", () => {
  const read = async (): Promise<Settings> => ({ appearance: large, pause_warned: false, onboarded: false, close_to_tray: null, status_off: [], notifications: N, history_import: true, history_limit_gb: null, history_limit_days: null, history_backup_dir: null, default_folder: null, browser: BR, files: FI, changes: CH });

  it("reads pull requests as off after that switch alone was turned off", async () => {
    const { api, sync } = setup(read);
    await sync.load();
    await sync.setChanges({ pull_requests: false, resolved_comments: false });
    expect(api.put).toHaveBeenLastCalledWith(ep, expect.objectContaining({ changes: { hide_whitespace: false, wrap: false, pull_requests: false, resolved_comments: false } }));
    expect(sync.changes).toEqual({ hide_whitespace: false, wrap: false, pull_requests: false, resolved_comments: false });
  });

  it("a change of resolved_comments alone reaches the store", async () => {
    const { sync } = setup(read);
    await sync.load();
    await sync.setChanges({ resolved_comments: true });
    expect(sync.changes?.resolved_comments).toBe(true);
  });

  it("keeps the same object when a save changed nothing in them", async () => {
    const { sync } = setup(read);
    await sync.load();
    const before = sync.changes;
    await sync.setChanges({ wrap: false });
    expect(sync.changes).toBe(before);
  });
});

describe("loadWithRetry", () => {
  const sleeps = () => {
    const waited: number[] = [];
    return { waited, sleep: async (ms: number) => void waited.push(ms) };
  };
  it("stops at the first success, after the waits that came before it", async () => {
    let calls = 0;
    const { waited, sleep } = sleeps();
    const got = await loadWithRetry(async () => (++calls < 3 ? Promise.reject(new Error("no core")) : "ok"), () => true, [10, 20, 40], sleep);
    expect(got).toBe("ok");
    expect(calls).toBe(3);
    expect(waited).toEqual([10, 20]);
  });
  it("rejects with the last error once every attempt failed", async () => {
    let calls = 0;
    const { waited, sleep } = sleeps();
    await expect(loadWithRetry(async () => Promise.reject(new Error(`try ${++calls}`)), () => true, [10, 20], sleep)).rejects.toThrow("try 3");
    expect(waited).toEqual([10, 20]);
  });
  it("tries no more once the caller is gone", async () => {
    let calls = 0;
    let live = true;
    const { sleep } = sleeps();
    await expect(
      loadWithRetry(
        async () => {
          calls++;
          live = false;
          throw new Error("no core");
        },
        () => live,
        [10, 20],
        sleep,
      ),
    ).rejects.toThrow("no core");
    expect(calls).toBe(1);
  });
  it("leaves the store read after a failed first read, so the rule can arrive later", async () => {
    let calls = 0;
    const { api, sync } = setup(async () => {
      if (++calls === 1) throw new Error("not yet");
      return { appearance: large, pause_warned: false, onboarded: false, close_to_tray: null, status_off: [], notifications: N, history_import: true, history_limit_gb: null, history_limit_days: null, history_backup_dir: null, default_folder: null, browser: { ...BR, cookies: "until-quit" as const }, files: FI, changes: CH };
    });
    await loadWithRetry(() => sync.load(), () => true, [0], async () => {});
    expect(api.get).toHaveBeenCalledTimes(2);
    expect(sync.readFailed).toBe(false);
    expect(sync.browser?.cookies).toBe("until-quit");
  });
});
