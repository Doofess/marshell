import type { Appearance } from "../../generated/Appearance";
import type { Endpoint } from "../../generated/Endpoint";
import type { Settings } from "../../generated/Settings";

export const READ_FAILED = "Couldn't read your settings, so nothing was saved. Try again in a moment.";

/** The settings could not be read, so a save was not sent: it would have replaced them with defaults. */
export class SettingsReadError extends Error {
  constructor() {
    super(READ_FAILED);
  }
}

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

  constructor(
    private api: SettingsApi,
    private getEndpoint: () => Promise<Endpoint>,
  ) {}

  /** Reads the settings from the core. Rejects if it can't be reached. */
  async load(): Promise<Appearance> {
    // Asked each time: a restarted core listens on a new endpoint.
    const s = await this.api.get(await this.getEndpoint());
    this.hasRead = true;
    return s.appearance;
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
    const s = await this.api.put(await this.getEndpoint(), { appearance: next });
    return s.appearance;
  }
}
