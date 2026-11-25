import type { FilesSettings } from "../../generated/FilesSettings";
import type { PaneTab } from "../../generated/PaneTab";
import { EDITOR_DEFAULTS, type EditorPrefs } from "../editor/editorPrefs";
import type { PanelId } from "../pane/panels";
import { currentEntry } from "../pane/paneState";
import type { ViewAction } from "../window/viewState";
import { relativeTo, type TreeRow } from "./treeModel";

/** What the announcer says after Collapse all folders. */
export const COLLAPSED = "Collapsed all folders.";

/** The `open` action's preview flag for an open from the tree: a preview tab only when the setting is on and the open isn't a keep (Enter, a double-click). */
export function treeOpen(o: { previewTabs: boolean; keep: boolean }): { preview?: true } {
  return o.previewTabs && !o.keep ? { preview: true } : {};
}

/** The root-relative path the tree reveals for the open file, or null when there is no root, no file, or the file is outside the project folder (or is the folder itself). */
export function followTarget(root: string | null, activePath: string | null, windows: boolean): string | null {
  if (root === null || activePath === null) return null;
  const rel = relativeTo(root, activePath, windows);
  return rel === null || rel === "" ? null : rel;
}

/** Whether the panel's preview tab holds an unsaved path: it is kept first, so the preview that follows cannot replace it. */
export function keepFirst(previewTab: PaneTab | undefined, unsaved: (path: string) => boolean): boolean {
  return previewTab !== undefined && unsaved(currentEntry(previewTab).path);
}

/** Whether the action puts another session on screen than the one there now: the text tabs of the one left are auto-saved first. */
export function leavingSession(action: ViewAction, onScreen: string | undefined): boolean {
  return action.type === "select" && action.tabId !== onScreen;
}

/** Only a file row opens. A link is a link to a folder and is never opened; a folder, "more" and "empty" rows are not files. */
export function rowOpens(row: Pick<TreeRow, "kind"> | undefined): boolean {
  return row?.kind === "file";
}

/** How a tree open was made. The tree's `open` move has `keep` for Enter and not for a click or Space; `pointer` says the last input was a click. */
export function openHow(keep: boolean, pointer: boolean): "click" | "enter" | "space" {
  if (keep) return "enter";
  return pointer ? "click" : "space";
}

/** The keyboard goes to the file after an Enter or a double-click; a click or Space leaves it in the tree. */
export function keyboardToFile(how: "click" | "enter" | "double" | "space"): boolean {
  return how === "enter" || how === "double";
}

/**
 * After an open from the tree: whether the narrow tree, which takes the file's place, closes, and where the keyboard goes (null: it
 * stays in the tree). In the narrow layout every open shows the file (decision 1); Space leaves the keyboard on the path bar's tree
 * button rather than in the editor (decision 7). In the wide layout the tree stays, and Enter or a double-click moves the keyboard to
 * the file. A file opened in Browser (an HTML file) takes the keyboard there on Enter and leaves the Files panel's tree as it is.
 */
export function afterOpen(
  how: "click" | "enter" | "double" | "space",
  o: { narrow: boolean; panel: PanelId },
): { close: boolean; focus: PanelId | "tree-button" | null } {
  const keep = keyboardToFile(how);
  if (o.panel !== "files") return { close: false, focus: keep ? o.panel : null };
  if (!o.narrow) return { close: false, focus: keep ? "files" : null };
  return { close: true, focus: how === "space" ? "tree-button" : "files" };
}

/**
 * Whether the narrow tree is up, once the open file or the layout has changed. The last file closing brings the tree back, as there
 * is nothing else to show. A file opened from anywhere (the tree, Go to file, a terminal link, another tab) takes the tree's place:
 * it is what was asked for. A panel that has just become narrow shows its file, and the tree only when it has none: a tree left up
 * from before would cover a file that may hold the keyboard. Otherwise it is as it was.
 *
 * A tree that is up and holds the keyboard stays up through both: a file an agent opened in the background, or a window made
 * narrower, must not take the tree away from under the keyboard. An open the person asked for moves the keyboard first (a tab, the
 * palette, the terminal), and the tree's own open has already taken the tree down.
 */
export function treeAfter(
  up: boolean,
  was: { path: string | null; narrow: boolean },
  now: { path: string | null; narrow: boolean },
  keyboardInTree = false,
): boolean {
  if (now.path !== was.path) return now.path === null || (up && keyboardInTree);
  if (now.narrow && !was.narrow) return now.path === null || (up && keyboardInTree);
  return up;
}

/** Whether the keyboard moves to the filter box as the tree opens: only the narrow tree opening over a file that holds it, as the file goes inert and would drop it to the page. */
export function filterTakesKeyboard(o: { narrow: boolean; opening: boolean; focusInFile: boolean }): boolean {
  return o.narrow && o.opening && o.focusInFile;
}

/**
 * What "Show file tree" and "Collapse all folders" go through, and whether the Files panel is shown first. A mounted panel's tree
 * holds the live state (the saved one is read only when it mounts), so a collapse goes through it whenever it is mounted; the tree
 * is toggled through it only when it is on screen. A panel on screen is never hidden by the command.
 */
export function filesCommandSteps(what: "tree" | "collapse", o: { mounted: boolean; shown: boolean }): { via: "panel" | "saved"; showPanel: boolean } {
  const via = o.mounted && (what === "collapse" || o.shown) ? "panel" : "saved";
  return { via, showPanel: !o.shown };
}

/** The editor's part of the Files settings, or the defaults until they were read. */
export function editorPrefsOf(settings: FilesSettings | null): EditorPrefs {
  if (settings === null) return EDITOR_DEFAULTS;
  const { tab_indents, word_wrap, indent_guides, whitespace, scroll_past_end, tab_size } = settings;
  return { tab_indents, word_wrap, indent_guides, whitespace, scroll_past_end, tab_size };
}

export type PreviewExtras = { prefs?: EditorPrefs; autoSave?: boolean; onReveal?: (path: string) => void; pathShown?: boolean };

/**
 * What a panel's Preview gets besides the tab: the Files panel's editor settings, Auto save, the file manager's button, and a
 * header with no name or folder (the path bar shows them). The Browser panel's Preview gets none of them.
 */
export function previewExtras(panel: PanelId, files: { prefs: EditorPrefs; autoSave: boolean; onReveal(path: string): void }): PreviewExtras {
  return panel === "files" ? { prefs: files.prefs, autoSave: files.autoSave, onReveal: files.onReveal, pathShown: true } : {};
}
