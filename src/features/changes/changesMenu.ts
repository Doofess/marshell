import type { MenuItem } from "../../components/ContextMenu/menuKeys";
import type { ChangesSettings } from "../../generated/ChangesSettings";
import type { BarButton, BarMenuItem } from "./pullModel";
import { CHANGES_DEFAULTS } from "./changesSettings";
import type { Item, Row, Section } from "./model";
import { threadCopy } from "./threadCopy";
import type { SendState } from "./threads";

export type SettingKey = keyof ChangesSettings;
const SETTING_LABELS: Record<SettingKey, string> = { hide_whitespace: "Hide whitespace changes", wrap: "Wrap long lines", pull_requests: "Show pull requests from GitHub", resolved_comments: "Show resolved and outdated comments" };
const NO_FILES = "No files are changed";
const UNREAD = "Your settings couldn't be read";
const LOADING = "Your settings aren't read yet";

export const settingLabel = (key: SettingKey): string => SETTING_LABELS[key];

/**
 * The Changes panel's ⋯ menu: Refresh, the review action (left out where `review` is null), folding all files, and the two switches.
 * The switches are disabled until the settings are read, and read as the defaults after a read that failed.
 */
export function changesMenuItems(o: { settings: ChangesSettings | null; failed: boolean; review: string | null; hasFiles: boolean; github: BarMenuItem[]; main: BarButton | null }): MenuItem[] {
  const s = o.settings ?? (o.failed ? CHANGES_DEFAULTS : null);
  const noFiles = o.hasFiles ? {} : { disabled: NO_FILES };
  const unread = o.settings === null ? { disabled: o.failed ? UNREAD : LOADING } : {};
  const setting = (key: SettingKey): MenuItem => ({ id: `setting:${key}`, label: SETTING_LABELS[key], ...(s ? { checked: s[key] } : {}), ...unread });
  return [
    { id: "refresh", label: "Refresh" },
    ...(o.review === null ? [] : [{ id: "review", label: o.review }]),
    ...githubRows(o.github, o.main),
    { separator: true },
    { id: "fold-all", label: "Fold all files", ...noFiles },
    { id: "unfold-all", label: "Unfold all files", ...noFiles },
    { separator: true },
    setting("hide_whitespace"),
    setting("wrap"),
    setting("resolved_comments"),
  ];
}

const MAIN_ACTIONS: readonly string[] = ["push", "create", "merge", "auto_merge", "ready"];
const MENU_IDS: readonly string[] = ["push", "merge", "draft", "cancel_auto_merge", "hide", "ask_open", "ask_switch", "copy_switch", "add_unresolved", "open_on_github", "copy_link"];

/** The pull request bar's rows, under a separator: its main button when that is a pull request action, then its ⋯ rows. None gives nothing. */
function githubRows(menu: BarMenuItem[], main: BarButton | null): MenuItem[] {
  const first = main !== null && MAIN_ACTIONS.includes(main.id) && !menu.some((m) => m.id === main.id) ? [main] : [];
  if (first.length + menu.length === 0) return [];
  const row = (id: string, label: string, disabled: string | null): MenuItem => ({ id, label, ...(disabled === null ? {} : { disabled }) });
  return [{ separator: true }, ...first.map((b) => row(`github:main:${b.id}`, b.label, b.disabled)), ...menu.map((m) => row(`github:menu:${m.id}`, m.label, m.disabled))];
}

/** What a ⋯ menu row id asks for; null for an id the menu doesn't hold. */
export type ChangesChoice = { kind: "refresh" | "review" | "fold-all" | "unfold-all" } | { kind: "setting"; key: SettingKey } | { kind: "github"; from: "main" | "menu"; id: string };
export function changesChoice(id: string): ChangesChoice | null {
  if (id === "refresh" || id === "review" || id === "fold-all" || id === "unfold-all") return { kind: id };
  const [head, from, rest] = id.split(":");
  if (head === "github" && rest !== undefined) {
    if (from === "main" && MAIN_ACTIONS.includes(rest)) return { kind: "github", from, id: rest };
    if (from === "menu" && MENU_IDS.includes(rest)) return { kind: "github", from, id: rest };
    return null;
  }
  if (id.startsWith("setting:")) {
    const key = id.slice("setting:".length);
    if (key === "hide_whitespace" || key === "wrap" || key === "resolved_comments") return { kind: "setting", key };
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

/**
 * What a double-click on an item opens: what the menu's Open opens (a header at no line, a row at `openLineOf`'s), or the menu's
 * reason for a deleted file. Null for an item that opens nothing (a comment, a note, "Show"), or none.
 */
export function openOf(item: Item | undefined): { file: Section; line: number | undefined } | { refused: string } | null {
  if (item === undefined || (item.kind !== "file" && item.kind !== "row")) return null;
  if (item.file.status === "deleted") return { refused: `${DELETED}.` };
  return { file: item.file, line: item.kind === "row" ? openLineOf(item.file.rows, item.at) : undefined };
}

/** The menu of a row: `count` is how many lines a range holds, so Copy reads "Copy lines". */
export function lineMenuItems(item: Item, count = 1): MenuItem[] {
  const row = item.kind === "row" ? item.file.rows[item.at] : undefined;
  const deleted = "file" in item && item.file.status === "deleted";
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

/** An item of GitHub's comments: a thread or a note. */
export type CommentItem = Extract<Item, { kind: "thread" | "talk" }>;

/** The address of a thread's first comment or of a note, when it is an https one. */
export function addressOf(item: CommentItem): string | null {
  const url = item.kind === "thread" ? item.thread.comments[0]?.url : item.note.url;
  return typeof url === "string" && url.startsWith("https://") ? url : null;
}

/**
 * The menu of a thread or a note: add to or remove from the send; for a thread your note, a reply on GitHub (off with the reason when the
 * sign-in can't, or with "Replying…" while `replying`) and resolve or unresolve (off with the reason too); open and copy the address.
 * A note's own text isn't part of what a send carries of a GitHub note, so it has no note row.
 */
export function threadMenuItems(item: CommentItem, state: SendState, replying = false): MenuItem[] {
  const m = threadCopy.menu;
  const thread = item.kind === "thread" ? item.thread : null;
  const address = addressOf(item);
  const rows: MenuItem[] = [state.picked ? { id: "unpick", label: m.unpick } : { id: "pick", label: m.pick }];
  if (thread !== null) rows.push({ id: "note", label: state.note === "" ? m.note : m.editNote }, { id: "reply", label: m.reply, ...(!thread.can_reply ? { disabled: threadCopy.cantReply } : replying ? { disabled: threadCopy.replying } : {}) });
  if (thread !== null) rows.push(thread.resolved ? { id: "unresolve", label: m.unresolve, ...(thread.can_unresolve ? {} : { disabled: threadCopy.cantUnresolve }) } : { id: "resolve", label: m.resolve, ...(thread.can_resolve ? {} : { disabled: threadCopy.cantResolve }) });
  if (address !== null) rows.push({ id: "open", label: m.open }, { id: "copy-link", label: m.copyLink });
  return rows;
}
