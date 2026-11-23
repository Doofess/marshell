import type { MenuItem } from "../../components/ContextMenu/menuKeys";
import type { ChangesSettings } from "../../generated/ChangesSettings";
import { CHANGES_DEFAULTS } from "./changesSettings";
import type { Item, Row, Section } from "./model";

export type SettingKey = keyof ChangesSettings;
const SETTING_LABELS: Record<SettingKey, string> = { hide_whitespace: "Hide whitespace changes", wrap: "Wrap long lines" };
const NO_FILES = "No files are changed";
const UNREAD = "Your settings couldn't be read";
const LOADING = "Your settings aren't read yet";

export const settingLabel = (key: SettingKey): string => SETTING_LABELS[key];

/**
 * The Changes panel's ⋯ menu: Refresh, the review action (left out where `review` is null), folding all files, and the two switches.
 * The switches are disabled until the settings are read, and read as the defaults after a read that failed.
 */
export function changesMenuItems(o: { settings: ChangesSettings | null; failed: boolean; review: string | null; hasFiles: boolean }): MenuItem[] {
  const s = o.settings ?? (o.failed ? CHANGES_DEFAULTS : null);
  const noFiles = o.hasFiles ? {} : { disabled: NO_FILES };
  const unread = o.settings === null ? { disabled: o.failed ? UNREAD : LOADING } : {};
  const setting = (key: SettingKey): MenuItem => ({ id: `setting:${key}`, label: SETTING_LABELS[key], ...(s ? { checked: s[key] } : {}), ...unread });
  return [
    { id: "refresh", label: "Refresh" },
    ...(o.review === null ? [] : [{ id: "review", label: o.review }]),
    { separator: true },
    { id: "fold-all", label: "Fold all files", ...noFiles },
    { id: "unfold-all", label: "Unfold all files", ...noFiles },
    { separator: true },
    setting("hide_whitespace"),
    setting("wrap"),
  ];
}

/** What a ⋯ menu row id asks for; null for an id the menu doesn't hold. */
export type ChangesChoice = { kind: "refresh" | "review" | "fold-all" | "unfold-all" } | { kind: "setting"; key: SettingKey };
export function changesChoice(id: string): ChangesChoice | null {
  if (id === "refresh" || id === "review" || id === "fold-all" || id === "unfold-all") return { kind: id };
  if (id.startsWith("setting:")) {
    const key = id.slice("setting:".length);
    if (key === "hide_whitespace" || key === "wrap") return { kind: "setting", key };
  }
  return null;
}

/**
 * The new-file line a row opens at: its own; a removed row's is the next row's, else the one before it; a hunk's is the first after it.
 * Undefined when no row of the file has one.
 */
export function openLineOf(rows: Row[], at: number): number | undefined {
  const own = rows[at]?.newNo;
  if (own !== undefined) return own;
  for (let i = at + 1; i < rows.length; i++) if (rows[i]!.newNo !== undefined) return rows[i]!.newNo;
  for (let i = at - 1; i >= 0; i--) if (rows[i]!.newNo !== undefined) return rows[i]!.newNo;
  return undefined;
}

const DELETED = "The file is deleted, so there is nothing to open";

/** The menu of a row: `count` is how many lines a range holds, so Copy reads "Copy lines". */
export function lineMenuItems(item: Item, count = 1): MenuItem[] {
  const row = item.kind === "row" ? item.file.rows[item.at] : undefined;
  const deleted = item.kind !== "comment" && item.file.status === "deleted";
  return [
    { id: "comment", label: "Comment", ...(row?.kind === "hunk" ? { disabled: "A changed block can't take a comment" } : {}) },
    { id: "open", label: "Open in Files at this line", ...(deleted ? { disabled: DELETED } : {}) },
    { id: "copy", label: count > 1 ? "Copy lines" : "Copy line" },
  ];
}

/** The menu of a file's header. */
export function headerMenuItems(file: Section): MenuItem[] {
  return [
    { id: "open", label: "Open in Files", ...(file.status === "deleted" ? { disabled: DELETED } : {}) },
    { id: "fold", label: file.folded ? "Unfold" : "Fold", ...(file.rows.length === 0 ? { disabled: "The file has no lines to fold" } : {}) },
    { id: "copy-path", label: "Copy path" },
  ];
}
