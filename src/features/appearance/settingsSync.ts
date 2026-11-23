import type { Appearance } from "../../generated/Appearance";
import type { BrowserSettings } from "../../generated/BrowserSettings";
import type { Endpoint } from "../../generated/Endpoint";
import type { ChangesSettings } from "../../generated/ChangesSettings";
import type { FilesSettings } from "../../generated/FilesSettings";
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

const DEFAULT_CHANGES: ChangesSettings = { hide_whitespace: false, wrap: false, pull_requests: true };
const DEFAULT_FILES: FilesSettings = { preview_tabs: true, auto_save: false, tab_indents: true, word_wrap: false, indent_guides: true, whitespace: false, scroll_past_end: false, tab_size: 4, tree_position: "automatic" };

const DEFAULT_NOTIFY: NotifySettings = {
  needs_you: { banner: true, sound: true },
  done: { banner: true, sound: true },
  error: { banner: true, sound: true },
  subagent_sound: false,
  quiet_hours: { enabled: false, start: "22:00", end: "08:00" },
  terminal_alerts: true,
};

type FlagChange = Partial<Pick<Settings, "pause_warned" | "onboarded" | "close_to_tray" | "status_off" | "notifications" | "history_import" | "history_limit_gb" | "history_limit_days" | "history_backup_dir" | "browser" | "files" | "changes">>;

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
  /** Keeps the current object when the fields are equal, so a store snapshot stays stable. */
  private setChangesValue(next: ChangesSettings): void {
    const cur = this.changesValue;
    if (cur.hide_whitespace === next.hide_whitespace && cur.wrap === next.wrap) return;
    this.changesValue = next;
  }

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
  /** The Files settings. Kept from the last read, so every save sends them back unchanged. */
  private filesValue: FilesSettings = DEFAULT_FILES;
  /** The Changes settings. Kept from the last read, so every save sends them back unchanged. */
  private changesValue: ChangesSettings = DEFAULT_CHANGES;
  /** Every settings write in order: the last one's end. Never rejects. */
  private queue: Promise<void> = Promise.resolve();
  /** The Changes settings, as of the last read or write: null until a read has worked. The same object until it changes. */
  get changes(): ChangesSettings | null {
    return this.hasRead ? this.changesValue : null;
  }

  /** Saves the changed Changes fields. Reads first, like `save`, and sends every other field back unchanged. Queued with every other write. */
  setChanges(change: Partial<ChangesSettings>): Promise<void> {
    return this.queued(async () => {
      await this.readFirst();
      await this.write({ changes: { ...this.changesValue, ...change } });
    });
  }

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
    this.setFilesValue(s.files);
    this.setChangesValue(s.changes);
    this.appearance = s.appearance;
    this.emit();
    return s.appearance;
  }

  /** The browser settings, as of the last read or write: null until a read has worked. The same object until it changes. */
  get browser(): BrowserSettings | null {
    return this.hasRead ? this.browserValue : null;
  }

  /** The Files settings, as of the last read or write: null until a read has worked. The same object until it changes. */
  get files(): FilesSettings | null {
    return this.hasRead ? this.filesValue : null;
  }

  /** Saves the changed Files fields. Reads first, like `save`, and sends every other field back unchanged. Queued with every other write. */
  setFiles(change: Partial<FilesSettings>): Promise<void> {
    return this.queued(async () => {
      await this.readFirst();
      await this.write({ files: { ...this.filesValue, ...change } });
    });
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

  /**
   * Runs one settings write after every earlier one has ended, so one write is in flight at a time and each sends the values the
   * one before it stored. A write that fails doesn't stop the next, and each caller gets its own result or rejection. Never call it
   * from inside a queued unit: that would wait on itself.
   */
  private queued<T>(run: () => Promise<T>): Promise<T> {
    const result = this.queue.then(run);
    this.queue = result.then(
      () => {},
      () => {},
    );
    return result;
  }

  /**
   * Saves the changed browser fields. Reads first, like `save`, and sends every other field back unchanged. It goes through the
   * same queue as every other write, so it is built from the value the write before it stored, and no write overlapping it sends
   * the old browser block back. A change that fails doesn't stop the next.
   */
  setBrowser(change: Partial<BrowserSettings>): Promise<void> {
    return this.queued(async () => {
      await this.readFirst();
      await this.write({ browser: { ...this.browserValue, ...change } });
    });
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

  /** Keeps the current object when the fields are equal, so a store snapshot stays stable. */
  private setFilesValue(next: FilesSettings): void {
    const cur = this.filesValue;
    if ((Object.keys(next) as (keyof FilesSettings)[]).every((k) => cur[k] === next[k]) && Object.keys(cur).length === Object.keys(next).length) return;
    this.filesValue = next;
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

  /** Saves `next` and returns what the core stored. Rejects with a SettingsReadError when nothing has been read yet and a read fails. Queued with every other write. */
  save(next: Appearance): Promise<Appearance> {
    return this.queued(() => this.saveNow(next));
  }

  private async saveNow(next: Appearance): Promise<Appearance> {
    if (!this.hasRead) {
      try {
        await this.load();
      } catch {
        throw new SettingsReadError();
      }
    }
    const s = await this.api.put(await this.getEndpoint(), { appearance: next, pause_warned: this.warned, onboarded: this.seen, close_to_tray: this.tray, status_off: this.off, notifications: this.notify, history_import: this.importOn, history_limit_gb: this.limitGb, history_limit_days: this.limitDays, history_backup_dir: this.backupDir, browser: this.browserValue, files: this.filesValue, changes: this.changesValue });
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
    this.setFilesValue(s.files);
    this.setChangesValue(s.changes);
    this.appearance = s.appearance;
    this.emit();
    return s.appearance;
  }

  /** Saves the notification settings. Reads first, like `save`, and sends every other field back unchanged. */
  async setNotifications(next: NotifySettings): Promise<void> {
    await this.writeFlags({ notifications: next });
  }

  /** Switches one agent's live status on or off. Reads first, like `save`, and sends every other field back unchanged. */
  setStatusOff(agent: string, off: boolean): Promise<void> {
    return this.queued(async () => {
      const rest = (await this.readFirst()).filter((id) => id !== agent);
      await this.write({ status_off: off ? [...rest, agent] : rest });
    });
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

  /** Writes the changed fields through the queue. */
  private writeFlags(change: FlagChange): Promise<void> {
    return this.queued(() => this.write(change));
  }

  /** The write itself, for a queued unit to call: it reads first when no read has worked. */
  private async write(change: FlagChange): Promise<void> {
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
      browser: this.browserValue, files: this.filesValue, changes: this.changesValue,
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
    this.setFilesValue(s.files);
    this.setChangesValue(s.changes);
    this.appearance = s.appearance;
    this.emit();
  }
}

/** The waits, in milliseconds, between a failed start-up read of the settings and each new attempt: five attempts over about 15 seconds. */
export const LOAD_RETRY_WAITS: readonly number[] = [1000, 2000, 4000, 8000];

/**
 * Runs `load`, and after a failure runs it again after each wait in `waits`, stopping at the first success or once `live()` is
 * false. Resolves with the first success; rejects with the last error when every attempt failed, or with the failure in hand when
 * `live()` went false. A failed read keeps the Browser pages private (decision 28 of slice 6r-1b), so a core that is only slow to
 * start must not pin that for the whole run.
 */
export async function loadWithRetry<T>(
  load: () => Promise<T>,
  live: () => boolean,
  waits: readonly number[] = LOAD_RETRY_WAITS,
  sleep: (ms: number) => Promise<void> = (ms) => new Promise((r) => setTimeout(r, ms)),
): Promise<T> {
  for (let i = 0; ; i++) {
    try {
      return await load();
    } catch (e) {
      if (i >= waits.length || !live()) throw e;
      await sleep(waits[i]!);
      if (!live()) throw e;
    }
  }
}
