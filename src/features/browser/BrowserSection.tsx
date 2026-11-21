import { Switch } from "../../components/Switch/Switch";
import type { HookInstall } from "../../generated/HookInstall";
import type { Platform } from "../../generated/Platform";
import type { CookieRule } from "../../generated/CookieRule";
import type { BrowserSettings } from "../../generated/BrowserSettings";
import type { HookAgent } from "../../lib/api/agents";
import { Choice } from "../appearance/AppearanceSections";
import { PAGES_UNAVAILABLE } from "../pane/EmptyBrowser";
import { autoOpenList } from "./autoOpen";
import { toggledText } from "./browserMenu";
import { useBrowserSettings } from "./browserSettings";
import { COOKIE_ROWS, cookiesSaid, needsReload, SETTINGS_UNREAD } from "./cookies";
import "../settings/Settings.css";
import "../appearance/AppearanceSections.css";

const LINKS = "Open terminal links here";
const AUTO = "Open HTML files agents write";
const NOT_SAVED = "The setting wasn't saved.";

const COOKIE_LINES: Record<CookieRule, string> = {
  always: "Sign-ins last until you clear browsing data.",
  "until-quit": "Cookies and site storage are cleared when Marshell quits and when it starts.",
  never: "Every Browser tab is a private page. Nothing is written to disk, and it is gone when the last Browser tab closes.",
};

type Props = {
  announce: (text: string) => void;
  /** False where the shell has no browser tabs. */
  pages: boolean;
  platform: Platform;
  /** The ids of the agent CLIs found on this machine. */
  found: readonly string[];
  hooks: Partial<Record<HookAgent, HookInstall>>;
};

/** The Browser settings that aren't a tab's own: the same saved values the Browser panel's menu changes. */
export function BrowserSection({ announce, pages, platform, found, hooks }: Props) {
  const { settings, failed, set } = useBrowserSettings();
  const unread = settings === null ? (failed ? SETTINGS_UNREAD : undefined) : undefined;
  const loading = settings === null;

  const toggle = (key: "terminal_links" | "open_agent_html", label: string, on: boolean): void => {
    set({ [key]: on } as Partial<BrowserSettings>).then(
      () => announce(toggledText(label, on)),
      () => announce(NOT_SAVED),
    );
  };
  const pick = (rule: CookieRule): void => {
    const was = settings?.cookies ?? null;
    // A change to or from "Never" is announced by the pages' own reload (useBrowserPages), with the count.
    set({ cookies: rule }).then(
      () => needsReload(was, rule) || announce(cookiesSaid(rule, 0)),
      () => announce(NOT_SAVED),
    );
  };

  const modifier = platform === "macos" ? "⌘" : "Ctrl";
  const wsl = platform === "windows" ? " In a WSL session, only files on a Windows drive (under /mnt) open." : "";
  const list = autoOpenList(found, hooks);
  const linksReason = !pages ? PAGES_UNAVAILABLE : unread;

  return (
    <div className="appearance">
      <div className="appearance__section">
        <Switch label={LINKS} checked={pages && settings?.terminal_links === true} disabled={!pages || loading} reason={linksReason} onChange={(on) => toggle("terminal_links", LINKS, on)} />
        <p className="field__help">{`${modifier}+click on a web link in terminal output opens it in the Browser panel. When this is off, it opens your system browser.`}</p>
      </div>
      <div className="appearance__section">
        <Switch label={AUTO} checked={settings?.open_agent_html === true} disabled={loading} reason={unread} onChange={(on) => toggle("open_agent_html", AUTO, on)} />
        <p className="field__help">{`An HTML file an agent creates or edits opens as a background tab in its session's Browser panel. A file written by a shell command isn't seen.${wsl}`}</p>
        {list.length === 0 ? (
          <p className="field__help">No agent CLI was found.</p>
        ) : (
          <ul className="appearance__lines">
            {list.map((row) => (
              <li key={row.agent} className="field__help">{`${row.label}: ${row.text}`}</li>
            ))}
          </ul>
        )}
      </div>
      <div className="appearance__section">
        <Choice
          legend="Cookies"
          name="browser-cookies"
          value={settings?.cookies ?? null}
          options={COOKIE_ROWS.map((r) => [r.rule, r.label] as const)}
          lines={COOKIE_LINES}
          disabled={linksReason ?? (loading ? "" : undefined)}
          onPick={pick}
        />
      </div>
    </div>
  );
}
