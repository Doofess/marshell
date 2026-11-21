import type { Appearance } from "../../generated/Appearance";
import type { BrowserSettings } from "../../generated/BrowserSettings";
import type { Endpoint } from "../../generated/Endpoint";
import type { NotifySettings } from "../../generated/NotifySettings";
import type { RetentionLimits } from "../../generated/RetentionLimits";
import type { Settings } from "../../generated/Settings";

export const READ_FAILED = "Couldn't read your settings, so nothing was saved. Try again in a moment.";

/** The settings could not be read, so a save was not sent: it would have replaced them with defaults. */
export class SettingsReadError extends Error {
  constructor() {
    super(READ_FAILED);
  }
}

const DEFAULT_BROWSER: BrowserSettings = { terminal_links: true, open_agent_html: true, cookies: "always" };

const DEFAULT_NOTIFY: NotifySettings = {
  needs_you: { banner: true, sound: true },
  done: { banner: true, sound: true },
  error: { banner: true, sound: true },
  subagent_sound: false,
  quiet_hours: { enabled: false, start: "22:00", end: "08:00" },
  terminal_alerts: true,
};

type SettingsApi = {
  get(ep: Endpoint): Promise<Settings>;
  put(ep: Endpoint, s: Settings): Promise<Settings>;
};

/**
 * Reads and saves the Appearance settings. Until one read has worked, a save reads first and sends nothing if that fails:
 * the screen is showing defaults then, and saving them would overwrite what is in the file.
 */
export class SettingsSync {
  private hasRead = false;
  /** Kept from the last read, so an appearance save sends it back unchanged. */
  private warned = false;
  /** Kept from the last read, so every save sends it back unchanged. */
  private seen = false;
  /** Whether closing hides the window to the tray: null until the person chooses. Kept from the last read, so every save sends it back unchanged. */
  private tray: boolean | null = null;
  /** Kept from the last read, so every save sends it back unchanged. */
  private off: string[] = [];
  /** Kept from the last read, so every save sends it back unchanged. */
  private notify: NotifySettings = DEFAULT_NOTIFY;
  /** Whether conversations run outside Marshell are copied. Kept from the last read, so every save sends it back unchanged. */
  private importOn = true;
  /** The size limit on recordings and images, in GB. Kept from the last read, so every save sends it back unchanged. */
  private limitGb: number | null = null;
  /** The age limit on recordings and images, in days. Kept from the last read, so every save sends it back unchanged. */
  private limitDays: number | null = null;
  /** The backup folder for the history. Kept from the last read, so every save sends it back unchanged. */
  private backupDir: string | null = null;
  /** The Browser panel settings. Kept from the last read, so every save sends them back unchanged. */
  private browserValue: BrowserSettings = DEFAULT_BROWSER;
  /** True after a read that failed, until one works. */
  private failed = false;
  private listeners = new Set<() => void>();
  /** The appearance the core last stored, which a pause-warning write sends back unchanged. */
  private appearance: Appearance | null = null;

  constructor(
    private api: SettingsApi,
    private getEndpoint: () => Promise<Endpoint>,
  ) {}

  /** Reads the settings from the core. Rejects if it can't be reached. */
  async load(): Promise<Appearance> {
    // Asked each time: a restarted core listens on a new endpoint.
    let s: Settings;
    try {
      s = await this.api.get(await this.getEndpoint());
    } catch (e) {
      this.failed = true;
      this.emit();
      throw e;
    }
    this.hasRead = true;
    this.failed = false;
    this.warned = s.pause_warned;
    this.seen = s.onboarded;
    this.tray = s.close_to_tray;
    this.off = s.status_off;
    this.notify = s.notifications;
    this.importOn = s.history_import;
    this.limitGb = s.history_limit_gb;
    this.limitDays = s.history_limit_days;
    this.backupDir = s.history_backup_dir;
    this.setBrowserValue(s.browser);
    this.appearance = s.appearance;
    this.emit();
    return s.appearance;
  }

  /** The browser settings, as of the last read or write: null until a read has worked. The same object until it changes. */
  get browser(): BrowserSettings | null {
    return this.hasRead ? this.browserValue : null;
  }

  /** True after a read that failed, until one works. */
  get readFailed(): boolean {
    return this.failed;
  }

  /** Calls `listener` after every read and write, a failed read too. Returns the function that stops it. */
  subscribe(listener: () => void): () => void {
    this.listeners.add(listener);
    return () => {
      this.listeners.delete(listener);
    };
  }

  /** Saves the changed browser fields. Reads first, like `save`, and sends every other field back unchanged. */
  async setBrowser(change: Partial<BrowserSettings>): Promise<void> {
    await this.readFirst();
    await this.writeFlags({ browser: { ...this.browserValue, ...change } });
  }

  private emit(): void {
    for (const l of [...this.listeners]) l();
  }

  /** Keeps the current object when the fields are equal, so a store snapshot stays stable. */
  private setBrowserValue(next: BrowserSettings): void {
    const cur = this.browserValue;
    if (cur.terminal_links === next.terminal_links && cur.open_agent_html === next.open_agent_html && cur.cookies === next.cookies) return;
    this.browserValue = next;
  }

  /** Whether conversations run outside Marshell are copied, as of the last read or write. True until a read works. */
  get historyImport(): boolean {
    return this.importOn;
  }

  /** The size and age limits on recordings and images, as of the last read or write. No limits until a read works. */
  get historyLimits(): RetentionLimits {
    return { limit_gb: this.limitGb, limit_days: this.limitDays };
  }

  /** The backup folder for the history, as of the last read or write: null when there is none. */
  get historyBackupDir(): string | null {
    return this.backupDir;
  }

  /** The notification settings, as of the last read or write. The defaults until a read works. */
  get notifications(): NotifySettings {
    return this.notify;
  }

  /** The agent ids whose live status is switched off, as of the last read or write. Empty until a read works. */
  get statusOff(): string[] {
    return this.off;
  }

  /** Whether the onboarding was seen, as of the last read or write. False until a read works. */
  get onboarded(): boolean {
    return this.seen;
  }

  /** Whether closing hides the window to the tray, as of the last read or write: null until the person has chosen (or a read works). */
  get closeToTray(): boolean | null {
    return this.tray;
  }

  /** Whether the first-Pause warning has been confirmed, as of the last read or write. False until a read works. */
  get pauseWarned(): boolean {
    return this.warned;
  }

  /** Saves `next` and returns what the core stored. Rejects with a SettingsReadError when nothing has been read yet and a read fails. */
  async save(next: Appearance): Promise<Appearance> {
    if (!this.hasRead) {
      try {
        await this.load();
      } catch {
        throw new SettingsReadError();
      }
    }
    const s = await this.api.put(await this.getEndpoint(), { appearance: next, pause_warned: this.warned, onboarded: this.seen, close_to_tray: this.tray, status_off: this.off, notifications: this.notify, history_import: this.importOn, history_limit_gb: this.limitGb, history_limit_days: this.limitDays, history_backup_dir: this.backupDir, browser: this.browserValue });
    this.warned = s.pause_warned;
    this.seen = s.onboarded;
    this.tray = s.close_to_tray;
    this.off = s.status_off;
    this.notify = s.notifications;
    this.importOn = s.history_import;
    this.limitGb = s.history_limit_gb;
    this.limitDays = s.history_limit_days;
    this.backupDir = s.history_backup_dir;
    this.setBrowserValue(s.browser);
    this.appearance = s.appearance;
    this.emit();
    return s.appearance;
  }

  /** Saves the notification settings. Reads first, like `save`, and sends every other field back unchanged. */
  async setNotifications(next: NotifySettings): Promise<void> {
    await this.writeFlags({ notifications: next });
  }

  /** Switches one agent's live status on or off. Reads first, like `save`, and sends every other field back unchanged. */
  async setStatusOff(agent: string, off: boolean): Promise<void> {
    const rest = (await this.readFirst()).filter((id) => id !== agent);
    await this.writeFlags({ status_off: off ? [...rest, agent] : rest });
  }

  /** Records that the first-Pause warning was confirmed. Reads first, like `save`, and sends the appearance last read back unchanged. */
  async markPauseWarned(): Promise<void> {
    await this.writeFlags({ pause_warned: true });
  }

  /** Saves whether closing hides the window to the tray. Reads first, like `save`, and sends every other field back unchanged. */
  async setCloseToTray(value: boolean): Promise<void> {
    await this.writeFlags({ close_to_tray: value });
  }

  /** Saves whether conversations run outside Marshell are copied. Reads first, like `save`, and sends every other field back unchanged, so a later appearance save keeps it. */
  async setHistoryImport(on: boolean): Promise<void> {
    await this.writeFlags({ history_import: on });
  }

  /** Saves the limits on recordings and images. Reads first, like `save`, and sends every other field back unchanged. A refusal (400) rejects with the core's body as the message. */
  async setHistoryLimits(limits: RetentionLimits): Promise<void> {
    await this.writeFlags({ history_limit_gb: limits.limit_gb, history_limit_days: limits.limit_days });
  }

  /** Saves the backup folder, or clears it with null. Reads first, like `save`, and sends every other field back unchanged. A refusal (400) rejects with the core's body as the message. */
  async setHistoryBackupDir(dir: string | null): Promise<void> {
    await this.writeFlags({ history_backup_dir: dir });
  }

  /** Records that the onboarding was seen. Reads first, like `save`, and sends the appearance and the pause warning back unchanged. */
  async markOnboarded(): Promise<void> {
    await this.writeFlags({ onboarded: true });
  }

  /** The off list from a read that has worked: reads first when none has. */
  private async readFirst(): Promise<string[]> {
    if (!this.hasRead) {
      try {
        await this.load();
      } catch {
        throw new SettingsReadError();
      }
    }
    return this.off;
  }

  private async writeFlags(change: Partial<Pick<Settings, "pause_warned" | "onboarded" | "close_to_tray" | "status_off" | "notifications" | "history_import" | "history_limit_gb" | "history_limit_days" | "history_backup_dir" | "browser">>): Promise<void> {
    if (!this.hasRead) {
      try {
        await this.load();
      } catch {
        throw new SettingsReadError();
      }
    }
    const s = await this.api.put(await this.getEndpoint(), {
      appearance: this.appearance!,
      pause_warned: this.warned,
      onboarded: this.seen,
      close_to_tray: this.tray,
      status_off: this.off,
      notifications: this.notify,
      history_import: this.importOn,
      history_limit_gb: this.limitGb,
      history_limit_days: this.limitDays,
      history_backup_dir: this.backupDir,
      browser: this.browserValue,
      ...change,
    });
    this.warned = s.pause_warned;
    this.seen = s.onboarded;
    this.tray = s.close_to_tray;
    this.off = s.status_off;
    this.notify = s.notifications;
    this.importOn = s.history_import;
    this.limitGb = s.history_limit_gb;
    this.limitDays = s.history_limit_days;
    this.backupDir = s.history_backup_dir;
    this.setBrowserValue(s.browser);
    this.appearance = s.appearance;
    this.emit();
  }
}
