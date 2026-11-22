import type { MenuItem } from "../../components/ContextMenu/menuKeys";
import type { FilesSettings } from "../../generated/FilesSettings";
import type { TreePosition } from "../../generated/TreePosition";
import { commandById, shortcutLabel, type Os } from "../../lib/keymap";

type SettingKey = "preview_tabs" | "auto_save" | "tab_indents" | "word_wrap" | "indent_guides" | "whitespace" | "scroll_past_end";

export type FilesChoice =
  | { kind: "goToFile" }
  | { kind: "tree" }
  | { kind: "position"; position: TreePosition }
  | { kind: "collapse" }
  | { kind: "setting"; key: SettingKey }
  | { kind: "tabSize"; size: 2 | 4 | 8 }
  | null;

const SETTING_LABELS: Record<SettingKey, string> = {
  preview_tabs: "Preview tabs",
  auto_save: "Auto save",
  tab_indents: "Tab key indents",
  word_wrap: "Word wrap",
  indent_guides: "Indent guides",
  whitespace: "Show whitespace",
  scroll_past_end: "Scroll past end",
};
const POSITIONS: { position: TreePosition; label: string }[] = [
  { position: "automatic", label: "Automatic" },
  { position: "left", label: "Left" },
  { position: "right", label: "Right" },
];
const SIZES = [2, 4, 8] as const;
const UNREAD = "Your settings couldn't be read";

/**
 * The Files panel's ⋯ menu: Go to file…, the tree's rows, and the editor's settings. Until the settings are read no setting
 * row is checked; when the read failed they say so in a note.
 */
export function filesMenuItems(o: { settings: FilesSettings | null; failed: boolean; treeShown: boolean; os: Os }): MenuItem[] {
  const s = o.settings;
  const note = o.failed && s === null ? { note: UNREAD } : {};
  const setting = (key: SettingKey): MenuItem => ({
    id: `setting:${key}`,
    label: SETTING_LABELS[key],
    ...(s ? { checked: s[key] } : {}),
    ...note,
  });
  const goTo = commandById("open-file");
  return [
    { id: "go-to-file", label: "Go to file…", shortcut: goTo.binding ? shortcutLabel(goTo.binding, o.os) : undefined },
    { separator: true },
    { id: "tree", label: "Show file tree", checked: o.treeShown },
    {
      id: "position",
      label: "File tree position",
      submenu: true,
      rows: POSITIONS.map((p) => ({ id: `position:${p.position}`, label: p.label, radio: true as const, ...(s ? { checked: s.tree_position === p.position } : {}) })),
    },
    { separator: true },
    { id: "collapse", label: "Collapse all folders" },
    { separator: true },
    setting("preview_tabs"),
    setting("auto_save"),
    setting("tab_indents"),
    { separator: true },
    setting("word_wrap"),
    setting("indent_guides"),
    setting("whitespace"),
    setting("scroll_past_end"),
    {
      id: "tab-size",
      label: "Tab size",
      submenu: true,
      ...note,
      rows: SIZES.map((n) => ({ id: `tabsize:${n}`, label: String(n), radio: true as const, ...(s ? { checked: s.tab_size === n } : {}) })),
    },
  ];
}

/** What a menu row id asks for; null for an id the menu doesn't hold. */
export function filesMenuChoice(id: string): FilesChoice {
  if (id === "go-to-file") return { kind: "goToFile" };
  if (id === "tree") return { kind: "tree" };
  if (id === "collapse") return { kind: "collapse" };
  const [head, rest] = [id.slice(0, id.indexOf(":")), id.slice(id.indexOf(":") + 1)];
  if (head === "position") {
    const p = POSITIONS.find((x) => x.position === rest);
    return p ? { kind: "position", position: p.position } : null;
  }
  if (head === "tabsize") {
    const n = SIZES.find((x) => String(x) === rest);
    return n ? { kind: "tabSize", size: n } : null;
  }
  if (head === "setting") {
    return rest in SETTING_LABELS ? { kind: "setting", key: rest as SettingKey } : null;
  }
  return null;
}

/** What the announcer says after a choice: "Show file tree: off", "File tree position: Left", "Tab size: 2". */
export function filesSaid(choice: Exclude<FilesChoice, null | { kind: "goToFile" } | { kind: "collapse" }>, on?: boolean): string {
  const state = on ? "on" : "off";
  switch (choice.kind) {
    case "tree":
      return `Show file tree: ${state}`;
    case "position":
      return `File tree position: ${POSITIONS.find((p) => p.position === choice.position)!.label}`;
    case "setting":
      return `${SETTING_LABELS[choice.key]}: ${state}`;
    case "tabSize":
      return `Tab size: ${choice.size}`;
  }
}
