import type { DraftComment } from "../../generated/DraftComment";
import type { PullNote } from "../../generated/PullNote";
import type { ReviewThread } from "../../generated/ReviewThread";
import { place, rowsAnchor, type Placement } from "./comments";
import { copy } from "./copy";
import type { Row, Section } from "./model";
import { threadCopy } from "./threadCopy";

/** How many characters of a comment's text a label reads. */
const LABEL_CHARS = 200;
/** How many of a thread's comments a label reads. */
const LABEL_COMMENTS = 10;

const cut = (text: string, max: number) => {
  const points = Array.from(text);
  return points.length <= max ? text : points.slice(0, max).join("");
};

/**
 * What the core may take out of a thread's rows (`hides` and `strip_hidden` in `crates/core/src/github`): control characters but the tab,
 * and every character that draws nothing, with the joiner and the selector it keeps inside an emoji. The checkout's own diff keeps them (a
 * CR at each line's end, a byte order mark on the first), so a thread's rows and the diff's are compared without them.
 */
const UNSEEN =
  /[\u0000-\u0008\u000a-\u001f\u007f-\u009f\u{ad}\u{34f}\u{61c}\u{115f}\u{1160}\u{17b4}\u{17b5}\u{180b}-\u{180f}\u{200b}-\u{200f}\u{2028}-\u{202e}\u{2060}-\u{206f}\u{3164}\u{fe00}-\u{fe0f}\u{feff}\u{ffa0}\u{fff0}-\u{fffb}\u{1bca0}-\u{1bca3}\u{1d173}-\u{1d17a}\u{e0000}-\u{e0fff}]/gu;
const seen = <T extends Pick<Row, "kind" | "text">>(r: T): T => (r.kind === "hunk" ? r : { ...r, text: r.text.replace(UNSEEN, "") });
const seenRows = new WeakMap<Row[], Row[]>();
/** A section's rows without what `UNSEEN` names, kept per rows array (as `hashesOf` keeps its hashes). */
const comparable = (rows: Row[]): Row[] => {
  let c = seenRows.get(rows);
  if (!c) seenRows.set(rows, (c = rows.map(seen)));
  return c;
};

/** The thread as a draft comment's anchor: its rows without what `UNSEEN` names, hashed as `anchorOf` hashes them, and quoted as they were sent. Null for a file comment or a thread with no rows. */
export function threadAnchor(t: ReviewThread): DraftComment | null {
  if (t.file || t.rows.length === 0) return null;
  const end = t.end ?? t.original_end ?? 1;
  const start = t.start ?? t.original_start ?? end;
  return { id: t.id, path: t.path, side: t.side, start, end, ...rowsAnchor(t.rows.map(seen)), quote: rowsAnchor(t.rows).quote, text: "" };
}

/** Where the thread sits in its file's section: null when the diff has no such file (or the section has no rows); `{at: null, changed: false}` for a file comment; else `place(threadAnchor(t), file)` on the section's rows without what `UNSEEN` names. */
export function placeThread(t: ReviewThread, file: Section | undefined): Placement | null {
  if (file === undefined || file.rows.length === 0) return null;
  if (t.file) return { at: null, changed: false };
  const anchor = threadAnchor(t);
  return anchor === null ? { at: null, changed: true } : place(anchor, { ...file, rows: comparable(file.rows) });
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

/** The words read for a thread item: who, where, how many replies, the first comment's text (200 characters), then each mark that applies, then the user's own note (200 characters). */
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
  const all = [...threadMarks(t, placed, state, agent), ...(state.note === "" ? [] : [threadCopy.yourNote(cut(state.note, LABEL_CHARS))])];
  return all.length === 0 ? label : `${label}. ${all.join(". ")}`;
}

/** The marks that apply to a thread, in the order its label reads them and its chips show them. */
export function threadMarks(t: ReviewThread, placed: Placement | null, state: SendState, agent: string | null): string[] {
  const anchor = threadAnchor(t);
  const range = (placed?.start ?? anchor?.start) !== (placed?.end ?? anchor?.end);
  return [
    ...(t.resolved ? [threadCopy.resolved] : []),
    ...(t.outdated ? [threadCopy.outdated] : []),
    ...(placed?.changed ? [copy.changed(range)] : []),
    ...marks(t.comments.some((c) => c.hidden_removed), state, agent),
  ];
}

/** The marks that apply to a review or comment of the pull request. */
export const noteMarks = (n: PullNote, state: SendState, agent: string | null): string[] => marks(n.hidden_removed, state, agent);

/** "Review by alice · Changes requested: …" or "Comment by alice: …", then the marks that apply. */
export function noteLabel(n: PullNote, state: SendState, agent: string | null): string {
  const who = n.kind === "review" ? threadCopy.reviewBy(authorOf(n), n.verdict) : threadCopy.commentBy(authorOf(n));
  const all = noteMarks(n, state, agent);
  const label = `${who}: ${cut(n.body, LABEL_CHARS)}`;
  return all.length === 0 ? label : `${label}. ${all.join(". ")}`;
}
