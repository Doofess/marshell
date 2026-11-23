import type { ChangesDrafts } from "../../generated/ChangesDrafts";
import type { DraftComment } from "../../generated/DraftComment";
import { type BoxAnchor, type BoxState, anchorKey } from "./boxModel";
import { type Placement, messageOf, renumbered } from "./comments";
import type { Section } from "./model";
import { treeFolderOk } from "../pane/paneRules";

/** The checkout shown and the base it is compared with. */
export type Shown = { folder: string; base: string | null };

export const commentsOf = (n: number) => `${n} ${n === 1 ? "comment" : "comments"}`;
export const folderName = (path: string) => path.split(/[\\/]/).filter((s) => s !== "").pop() ?? path;

/** Whether the drafts' comments were written on another checkout than the one shown. */
const away = (drafts: ChangesDrafts | undefined, folder: string): boolean => drafts !== undefined && drafts.comments.length > 0 && drafts.checkout !== folder;

/** The drafts with `comments` in place of their own. While earlier comments exist they keep the checkout and base those were written against. */
export function withComments(drafts: ChangesDrafts | undefined, shown: Shown, comments: DraftComment[]): ChangesDrafts {
  return { ...withFolds(drafts, shown, {}), comments };
}

/** An open comment box: what it is about, its text, and the checkout and base shown when it was opened. */
export type OpenBox = { state: BoxState; text: string; on: Shown };

/**
 * The drafts once a box's text is kept. A new comment goes in at `anchor` (where its rows are now) with `id`, on the checkout the box
 * was opened on, so a shell that moved to another repository meanwhile doesn't take it along; an edit takes the new text, its lines
 * renumbered by `now`. An edit whose comment is gone meanwhile (deleted, or sent) goes in as a new one, so its text isn't lost.
 */
export function keptBox(drafts: ChangesDrafts | undefined, box: OpenBox, anchor: BoxAnchor, id: string, now: (c: DraftComment) => DraftComment): ChangesDrafts {
  const comments = drafts?.comments ?? [];
  const edits = box.state.id !== null && comments.some((c) => c.id === box.state.id);
  const next = edits ? comments.map((c) => (c.id === box.state.id ? { ...now(c), text: box.text } : c)) : [...comments, { ...anchor, id, text: box.text }];
  return withComments(drafts, box.on, next);
}

/**
 * What asking for a box does to the one that is open. "same": it is that box, which stays as it is, text and all. "keep": the open one
 * holds typed text, which is kept as a draft first, as Esc keeps it. "open": none is open, or it is empty.
 */
export function givesWay(open: OpenBox | null, next: BoxState): "open" | "same" | "keep" {
  if (open === null) return "open";
  const same = open.state.id !== null ? next.id === open.state.id : next.id === null && anchorKey(open.state.anchor) === anchorKey(next.anchor);
  if (same) return "same";
  return open.text.trim() === "" ? "open" : "keep";
}

/**
 * Where a box's comment is anchored: at the lines its rows are on now (`placed`), which a reply during the writing may have moved. A box
 * opened on another checkout than the one shown keeps the lines it was opened at: a file of the same name there isn't its own.
 */
export function boxAnchor(box: OpenBox, folder: string, placed: Placement | undefined): BoxAnchor {
  return box.on.folder === folder ? renumbered(box.state.anchor, placed) : box.state.anchor;
}

/**
 * The drafts with `folds` added. While comments exist they keep the checkout and base they were written against. With `inDiff` (the
 * files of the diff shown now) only those files' folds are kept, so the folds of files long gone never fill the layout's limit.
 */
export function withFolds(drafts: ChangesDrafts | undefined, shown: Shown, folds: Record<string, boolean>, inDiff?: ReadonlySet<string>): ChangesDrafts {
  const keep = drafts !== undefined && drafts.comments.length > 0;
  const all = { ...drafts?.folds, ...folds };
  return {
    checkout: keep ? drafts.checkout : shown.folder,
    base: keep ? drafts.base : shown.base,
    comments: drafts?.comments ?? [],
    folds: inDiff === undefined ? all : Object.fromEntries(Object.entries(all).filter(([path]) => inDiff.has(path))),
  };
}

/** The sentence for comments that belong to another checkout than the shown one, else null. */
export function elsewhere(drafts: ChangesDrafts | undefined, folder: string): string | null {
  if (drafts === undefined || !away(drafts, folder)) return null;
  return `Send or discard the ${commentsOf(drafts.comments.length)} on ${folderName(drafts.checkout)} first.`;
}

/** The comments the shown diff lists: none while they belong to another checkout, whose files the diff doesn't show. */
export function shownComments(drafts: ChangesDrafts | undefined, folder: string): DraftComment[] {
  return drafts === undefined || away(drafts, folder) ? [] : drafts.comments;
}

/** Why a comment or a fold on `path` can't be kept, else null: the layout keeps a path only as the core's rule for saved paths allows. */
export function unkept(path: string): string | null {
  return treeFolderOk(path) ? null : "This file's name holds a colon or a backslash, so a comment or a fold on it can't be kept.";
}

/** The message Send hands over. Another checkout's comments are cited at the lines they were written at, against its base, with the checkout named. */
export function sendMessageOf(drafts: ChangesDrafts, shown: Shown, sections: Section[], placements: Map<string, Placement>): string {
  if (away(drafts, shown.folder)) return messageOf(drafts.comments, new Map(), [], drafts.base, drafts.checkout);
  return messageOf(drafts.comments, placements, sections, shown.base);
}
