import type { FilesSettings } from "../../generated/FilesSettings";
import { FILES_DEFAULTS } from "../files/filesSettings";

/** The Files settings the editor itself reads: the keys, the drawing and the wrapping. (Auto save is the tab's.) */
export type EditorPrefs = Pick<FilesSettings, "tab_indents" | "word_wrap" | "indent_guides" | "whitespace" | "scroll_past_end" | "tab_size">;

const { tab_indents, word_wrap, indent_guides, whitespace, scroll_past_end, tab_size } = FILES_DEFAULTS;

/** What an editor uses until the settings have been read, or where nothing passes them. */
export const EDITOR_DEFAULTS: EditorPrefs = { tab_indents, word_wrap, indent_guides, whitespace, scroll_past_end, tab_size };
