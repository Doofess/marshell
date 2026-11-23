import type { ChangesDrafts } from "../../generated/ChangesDrafts";
import type { DraftComment } from "../../generated/DraftComment";
import type { BoxAnchor, BoxState } from "./boxModel";
import { type Placement, messageOf } from "./comments";
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
 * renumbered by `now`.
 */
export function keptBox(drafts: ChangesDrafts | undefined, box: OpenBox, anchor: BoxAnchor, id: string, now: (c: DraftComment) => DraftComment): ChangesDrafts {
  const comments = drafts?.comments ?? [];
  const next = box.state.id === null ? [...comments, { ...anchor, id, text: box.text }] : comments.map((c) => (c.id === box.state.id ? { ...now(c), text: box.text } : c));
  return withComments(drafts, box.on, next);
}

/** The drafts with `folds` added. While comments exist they keep the checkout and base they were written against. */
export function withFolds(drafts: ChangesDrafts | undefined, shown: Shown, folds: Record<string, boolean>): ChangesDrafts {
  const keep = drafts !== undefined && drafts.comments.length > 0;
  return {
    checkout: keep ? drafts.checkout : shown.folder,
    base: keep ? drafts.base : shown.base,
    comments: drafts?.comments ?? [],
    folds: { ...drafts?.folds, ...folds },
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
