import { Switch } from "../../components/Switch/Switch";
import type { FilesSettings } from "../../generated/FilesSettings";
import type { TreePosition } from "../../generated/TreePosition";
import { Choice } from "../appearance/AppearanceSections";
import { toggledText } from "../browser/browserMenu";
import { SETTINGS_UNREAD } from "../browser/cookies";
import { useFilesSettings } from "./filesSettings";
import "../settings/Settings.css";
import "../appearance/AppearanceSections.css";

const NOT_SAVED = "The setting wasn't saved.";

type Flag = "preview_tabs" | "auto_save" | "tab_indents" | "word_wrap" | "indent_guides" | "whitespace" | "scroll_past_end";

const FLAGS: ReadonlyArray<{ key: Flag; label: string; line: string }> = [
  { key: "preview_tabs", label: "Preview tabs", line: "A file opened from the tree shows in a tab that the next one replaces, until you edit it or double-click it." },
  { key: "auto_save", label: "Auto save", line: "Saves a file 1 second after you stop typing and when you leave its tab. It stops when the file changed on disk." },
  { key: "tab_indents", label: "Tab key indents", line: "When off, Tab moves to the next control. Esc, then Tab, always leaves the editor." },
  { key: "word_wrap", label: "Word wrap", line: "When on, every file wraps. When off, Markdown and plain text wrap and code doesn't. Alt+Z changes the tab in front." },
  { key: "indent_guides", label: "Indent guides", line: "Draws a line at each indent level." },
  { key: "whitespace", label: "Show whitespace", line: "Draws spaces as dots and tabs as arrows." },
  { key: "scroll_past_end", label: "Scroll past end", line: "Lets the last line scroll up to the top." },
];

const SIZES = [["2", "2"], ["4", "4"], ["8", "8"]] as const;
const POSITIONS: ReadonlyArray<readonly [TreePosition, string]> = [["automatic", "Automatic"], ["left", "Left"], ["right", "Right"]];

/** Saves one change and says so when it fails. */
export function saveFilesSetting(set: (c: Partial<FilesSettings>) => Promise<void>, announce: (text: string) => void, change: Partial<FilesSettings>, said: string | null): Promise<void> {
  return set(change).then(
    () => {
      if (said) announce(said);
    },
    () => announce(NOT_SAVED),
  );
}

/** The Files settings: the same saved values the Files panel's ⋯ menu changes. */
export function FilesSection({ announce }: { announce: (text: string) => void }) {
  const { settings, failed, set } = useFilesSettings();
  const loading = settings === null;
  const unread = loading ? (failed ? SETTINGS_UNREAD : undefined) : undefined;
  const off = unread ?? (loading ? "" : undefined);

  return (
    <div className="appearance">
      {FLAGS.map(({ key, label, line }) => (
        <div key={key} className="appearance__section">
          <Switch label={label} checked={settings?.[key] === true} disabled={loading} reason={unread} onChange={(on) => void saveFilesSetting(set, announce, { [key]: on }, toggledText(label, on))} />
          <p className="field__help">{line}</p>
        </div>
      ))}
      <div className="appearance__section">
        <Choice
          legend="Tab size"
          help="How wide a tab is drawn, and the indent of a file whose own can't be told."
          name="files-tab-size"
          value={settings ? String(settings.tab_size) : null}
          options={SIZES}
          disabled={off}
          onPick={(v) => void saveFilesSetting(set, announce, { tab_size: Number(v) }, `Tab size: ${v}`)}
        />
      </div>
      <div className="appearance__section">
        <Choice
          legend="File tree position"
          help="Automatic: At the window's edge."
          name="files-tree-position"
          value={settings?.tree_position ?? null}
          options={POSITIONS}
          disabled={off}
          onPick={(v) => void saveFilesSetting(set, announce, { tree_position: v }, `File tree position: ${POSITIONS.find((p) => p[0] === v)?.[1]}`)}
        />
      </div>
    </div>
  );
}
