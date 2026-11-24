import { Switch } from "../../components/Switch/Switch";
import type { ChangesSettings } from "../../generated/ChangesSettings";
import { toggledText } from "../browser/browserMenu";
import { SETTINGS_UNREAD } from "../browser/cookies";
import { useChangesSettings } from "./changesSettings";
import "../settings/Settings.css";
import "../appearance/AppearanceSections.css";

const NOT_SAVED = "The setting wasn't saved.";

type Flag = keyof ChangesSettings;

const FLAGS: ReadonlyArray<{ key: Flag; label: string; line: string }> = [
  { key: "hide_whitespace", label: "Hide whitespace changes", line: "Leaves out lines that differ only in spaces and tabs." },
  { key: "wrap", label: "Wrap long lines", line: "Wraps a long line in the diff instead of scrolling it sideways." },
  { key: "pull_requests", label: "Show pull requests from GitHub", line: "Asks GitHub through the GitHub CLI while Changes shows a checkout whose origin is on GitHub. Off, Marshell never runs gh." },
  { key: "resolved_comments", label: "Show resolved and outdated comments", line: "Shows a pull request's resolved and outdated review comments in the diff." },
];

/** Saves one change and says so when it fails. */
export function saveChangesSetting(set: (c: Partial<ChangesSettings>) => Promise<void>, announce: (text: string) => void, change: Partial<ChangesSettings>, said: string | null): Promise<void> {
  return set(change).then(
    () => {
      if (said) announce(said);
    },
    () => announce(NOT_SAVED),
  );
}

/** The Changes settings: the same saved values the Changes panel's menu changes. */
export function ChangesSection({ announce }: { announce: (text: string) => void }) {
  const { settings, failed, set } = useChangesSettings();
  const loading = settings === null;
  const unread = loading ? (failed ? SETTINGS_UNREAD : undefined) : undefined;

  return (
    <div className="appearance">
      {FLAGS.map(({ key, label, line }) => (
        <div key={key} className="appearance__section">
          <Switch label={label} checked={settings?.[key] === true} disabled={loading} reason={unread} onChange={(on) => void saveChangesSetting(set, announce, { [key]: on }, toggledText(label, on))} />
          <p className="field__help">{line}</p>
        </div>
      ))}
    </div>
  );
}
