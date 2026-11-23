import type { DraftComment } from "../../generated/DraftComment";
import type { PullNote } from "../../generated/PullNote";
import type { ReviewThread } from "../../generated/ReviewThread";
import { place, rowsAnchor, type Placement } from "./comments";
import { copy } from "./copy";
import type { Section } from "./model";
import { threadCopy } from "./threadCopy";

/** How many characters of a comment's text a label reads. */
const LABEL_CHARS = 200;
/** How many of a thread's comments a label reads. */
const LABEL_COMMENTS = 10;

const cut = (text: string, max: number) => {
  const points = Array.from(text);
  return points.length <= max ? text : points.slice(0, max).join("");
};

/** The thread as a draft comment's anchor: its rows hashed as `anchorOf` hashes them. Null for a file comment or a thread with no rows. */
export function threadAnchor(t: ReviewThread): DraftComment | null {
  if (t.file || t.rows.length === 0) return null;
  const end = t.end ?? t.original_end ?? 1;
  const start = t.start ?? t.original_start ?? end;
  return { id: t.id, path: t.path, side: t.side, start, end, ...rowsAnchor(t.rows), text: "" };
}

/** Where the thread sits in its file's section: null when the diff has no such file (or the section has no rows); `{at: null, changed: false}` for a file comment; else `place(threadAnchor(t), file)`. */
export function placeThread(t: ReviewThread, file: Section | undefined): Placement | null {
  if (file === undefined || file.rows.length === 0) return null;
  if (t.file) return { at: null, changed: false };
  const anchor = threadAnchor(t);
  return anchor === null ? { at: null, changed: true } : place(anchor, file);
}

/** Unresolved threads on current lines, in GitHub's order. */
export const toResolve = (threads: ReviewThread[]): ReviewThread[] => threads.filter((t) => !t.resolved && !t.outdated);

/** `toResolve`, or every thread when `showOld`. */
export const shownThreads = (threads: ReviewThread[], showOld: boolean): ReviewThread[] => (showOld ? threads : toResolve(threads));

export const commentCount = (t: ReviewThread): number => t.comments.length + t.more_comments;

/** How a thread or a note stands in the send. */
export type SendState = { picked: boolean; note: string; sent: boolean };

/** "alice", or "dependabot (bot)". */
export const authorOf = (c: { author: string; bot: boolean }): string => (c.bot ? threadCopy.bot(c.author) : c.author);

/** What the list needs of GitHub's comments. */
export type GithubItems = {
  /** The ones to show. */
  threads: ReviewThread[];
  /** By thread id. */
  placements: Map<string, Placement | null>;
  /** The reviews and comments, when the list is open. */
  notes: PullNote[];
  /** "And 2 more comment threads on GitHub." and the like. */
  lines: string[];
  state(id: string): SendState;
  agent: string | null;
};

const marks = (hidden: boolean, state: SendState, agent: string | null): string[] => [...(hidden ? [threadCopy.hiddenRemoved] : []), ...(state.picked ? [threadCopy.inSend] : []), ...(state.sent ? [threadCopy.sentTo(agent)] : [])];

/** The words read for a thread item: who, where, how many replies, the first comment's text (200 characters), then each mark that applies. */
export function threadLabel(t: ReviewThread, placed: Placement | null, state: SendState, agent: string | null): string {
  const anchor = threadAnchor(t);
  const start = placed?.start ?? anchor?.start;
  const end = placed?.end ?? anchor?.end;
  const where = t.file || start === undefined || end === undefined ? "the file" : start === end ? `line ${start}` : `lines ${start} to ${end}`;
  const first = t.comments[0];
  const replies = commentCount(t) - 1;
  let label = `GitHub comment by ${first === undefined ? "" : authorOf(first)} on ${where}`;
  if (t.comments.length > 1) label += `, ${replies === 1 ? "1 reply" : `${replies} replies`}`;
  if (first !== undefined) label += `: ${cut(first.body, LABEL_CHARS)}`;
  for (const c of t.comments.slice(1, LABEL_COMMENTS)) label += `. ${threadCopy.replyBy(authorOf(c))}: ${cut(c.body, LABEL_CHARS)}`;
  const more = Math.max(0, t.comments.length - LABEL_COMMENTS) + t.more_comments;
  if (more > 0) label += `. ${threadCopy.moreComments(more).replace(/\.$/, "")}`;
  const all = [
    ...(t.resolved ? [threadCopy.resolved] : []),
    ...(t.outdated ? [threadCopy.outdated] : []),
    ...(placed?.changed ? [copy.changed(start !== end)] : []),
    ...marks(t.comments.some((c) => c.hidden_removed), state, agent),
  ];
  return all.length === 0 ? label : `${label}. ${all.join(". ")}`;
}

/** "Review by alice · Changes requested: …" or "Comment by alice: …", then the marks that apply. */
export function noteLabel(n: PullNote, state: SendState, agent: string | null): string {
  const who = n.kind === "review" ? threadCopy.reviewBy(authorOf(n), n.verdict) : threadCopy.commentBy(authorOf(n));
  const all = marks(n.hidden_removed, state, agent);
  const label = `${who}: ${cut(n.body, LABEL_CHARS)}`;
  return all.length === 0 ? label : `${label}. ${all.join(". ")}`;
}
