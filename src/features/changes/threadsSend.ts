import type { ChangesDrafts } from "../../generated/ChangesDrafts";
import type { PickedComment } from "../../generated/PickedComment";
import type { PullNote } from "../../generated/PullNote";
import type { PullThreads } from "../../generated/PullThreads";
import type { ReviewThread } from "../../generated/ReviewThread";
import type { SentComment } from "../../generated/SentComment";
import { type Placement, QUOTE_LINES, citation, renumbered } from "./comments";
import { type Shown, withFolds } from "./drafts";
import { authorOf, commentCount, threadAnchor, toResolve } from "./threads";

/** How many GitHub threads and notes one send holds. */
export const PICKED_MAX = 200;
/** How many sent marks are kept. */
export const SENT_MAX = 500;
/** The UTF-8 bytes one send may hold; the core's send route refuses a text over 1 MiB. */
export const SEND_BYTES = 1_000_000;

// The message's own words: part of the format the agent reads, not UI copy.
const HEAD = (number: number) => `Review comments from GitHub on pull request #${number}, each under its author's name:`;
const MY_NOTE = "My note:";
const CUT_LINE = "    … (cut at 10,000 characters; the rest is on GitHub)";
const MORE_COMMENTS = (n: number) => `    (and ${n} more on GitHub)`;
const THE_FILE = "(the file)";
const REVIEW_BY = (author: string, verdict: string) => `Review by ${author}${verdict}:`;
const COMMENT_BY = (author: string) => `Comment by ${author}:`;
const VERDICTS = { approved: " (approved)", changes_requested: " (changes requested)", dismissed: " (dismissed)", commented: "" } as const;

const bytesOf = (text: string) => new TextEncoder().encode(text).length;
/** Every line indented by four spaces, an empty line staying empty: no line of a body can read as one of the message's own. */
const indented = (text: string) => text.split("\n").map((l) => (l === "" ? "" : `    ${l}`));

/** Whether the thread or note `id` is marked sent: an entry with its id and its comment count of now. */
export const isSent = (id: string, count: number, sent: readonly SentComment[]): boolean => sent.some((s) => s.id === id && s.count === count);

/** The drafts with `id` of pull request `number` picked (its note replaced when given). At 200 picked, the picks of other pull requests go first, oldest first; null when all 200 are this pull request's. */
export function withPicked(drafts: ChangesDrafts | undefined, shown: Shown, number: number, id: string, note?: string): ChangesDrafts | null {
  const base = withFolds(drafts, shown, {});
  if (base.picked.some((p) => p.id === id)) return note === undefined ? base : { ...base, picked: base.picked.map((p) => (p.id === id ? { ...p, note } : p)) };
  let picked = base.picked;
  if (picked.length >= PICKED_MAX) {
    const at = picked.findIndex((p) => p.number !== number);
    if (at < 0) return null;
    picked = picked.filter((_, i) => i !== at);
  }
  return { ...base, picked: [...picked, { id, number, note: note ?? "" }] };
}

export const withoutPicked = (drafts: ChangesDrafts, id: string): ChangesDrafts => ({ ...drafts, picked: drafts.picked.filter((p) => p.id !== id) });

/** `toResolve(threads.threads)` (unresolved, on current lines: never one the user can't see) that are neither picked nor sent, picked for `threads.number`; `added` counts them, `sent` the ones skipped as sent. Stops at 200 picked. */
export function addUnresolved(drafts: ChangesDrafts | undefined, shown: Shown, threads: PullThreads): { drafts: ChangesDrafts; added: number; sent: number } {
  const base = withFolds(drafts, shown, {});
  const sentHere = base.sent.filter((s) => s.number === threads.number);
  const picked = [...base.picked];
  let added = 0;
  let sent = 0;
  for (const t of toResolve(threads.threads)) {
    if (picked.some((p) => p.id === t.id)) continue;
    if (isSent(t.id, commentCount(t), sentHere)) {
      sent++;
      continue;
    }
    if (picked.length >= PICKED_MAX) break;
    picked.push({ id: t.id, number: threads.number, note: "" });
    added++;
  }
  return { drafts: { ...base, picked }, added, sent };
}

/** What the send holds of GitHub's: each id picked for `threads.number` that the reply still lists (a thread or a note), in the reply's order, threads first. Another pull request's picks wait for their own. */
export type PickedItem = { kind: "thread"; thread: ReviewThread; note: string } | { kind: "talk"; note: PullNote; mine: string };

export function pickedItems(drafts: ChangesDrafts | undefined, threads: PullThreads | null): PickedItem[] {
  if (drafts === undefined || threads === null) return [];
  const mine = new Map(drafts.picked.filter((p) => p.number === threads.number).map((p) => [p.id, p.note]));
  return [
    ...threads.threads.filter((t) => mine.has(t.id)).map((t): PickedItem => ({ kind: "thread", thread: t, note: mine.get(t.id)! })),
    ...threads.notes.filter((n) => mine.has(n.id)).map((n): PickedItem => ({ kind: "talk", note: n, mine: mine.get(n.id)! })),
  ];
}

/** What is left of `SEND_BYTES` for GitHub's part beside the drafts' part (its bytes by `TextEncoder`, and the two line ends between). */
export const sendBudget = (draftsPart: string | null): number => (draftsPart === null ? SEND_BYTES : SEND_BYTES - bytesOf(draftsPart) - 2);

function threadBlock(t: ReviewThread, note: string, placed: Placement | null): string {
  const anchor = threadAnchor(t);
  const lines: string[] = [];
  if (anchor === null) lines.push(`${t.path} ${THE_FILE}`);
  else {
    lines.push(citation(renumbered(anchor, placed ?? undefined), placed?.changed ?? true));
    lines.push(...anchor.quote.map((q) => `> ${q}`));
    if (anchor.rows > QUOTE_LINES) lines.push("> …");
  }
  for (const c of t.comments) {
    lines.push(`${authorOf(c)}:`, ...indented(c.body));
    if (c.cut) lines.push(CUT_LINE);
  }
  if (t.more_comments > 0) lines.push(MORE_COMMENTS(t.more_comments));
  if (note !== "") lines.push(...note.split("\n").map((l, i) => (i === 0 ? `${MY_NOTE} ${l}` : l === "" ? "" : `    ${l}`)));
  return lines.join("\n");
}

function noteBlock(n: PullNote): string {
  const head = n.kind === "review" ? REVIEW_BY(authorOf(n), n.verdict === null ? "" : VERDICTS[n.verdict]) : COMMENT_BY(authorOf(n));
  return [head, ...indented(n.body)].join("\n");
}

/**
 * The GitHub part of the message, at most `budget` UTF-8 bytes. An item that doesn't fit in what is left is left out and stays picked;
 * later ones that fit still go. `went` names what the text holds, each with its comment count (a note's is 1); `left` counts what
 * didn't fit; `text` is null when nothing went. `placements` are `placeThread`'s. Only a body, an author, a path, the anchor and the
 * user's note reach the text.
 */
export function githubMessageOf(items: PickedItem[], number: number, placements: Map<string, Placement | null>, budget: number): { text: string | null; went: { id: string; count: number }[]; left: number } {
  const head = HEAD(number);
  let used = bytesOf(head);
  const blocks: string[] = [];
  const went: { id: string; count: number }[] = [];
  for (const item of items) {
    const block = item.kind === "thread" ? threadBlock(item.thread, item.note, placements.get(item.thread.id) ?? null) : noteBlock(item.note);
    const cost = bytesOf(block) + 2;
    if (used + cost > budget) continue;
    used += cost;
    blocks.push(block);
    went.push(item.kind === "thread" ? { id: item.thread.id, count: commentCount(item.thread) } : { id: item.note.id, count: 1 });
  }
  return { text: blocks.length === 0 ? null : [head, ...blocks].join("\n\n"), went, left: items.length - went.length };
}

/** The whole message: the drafts' part (when there are drafts) and GitHub's (when there are picked items), two line ends between. */
export const batchMessageOf = (draftsPart: string | null, githubPart: string | null): string => [draftsPart, githubPart].filter((p): p is string => p !== null).join("\n\n");

/** The drafts once the send left: the picked ids in `went` become sent marks of pull request `number` with their counts; the oldest marks past 500 go. */
export function afterSendPicked(drafts: ChangesDrafts, number: number, went: readonly { id: string; count: number }[]): ChangesDrafts {
  const ids = new Set(went.map((w) => w.id));
  const sent: SentComment[] = [...drafts.sent.filter((s) => !ids.has(s.id)), ...went.map((w) => ({ id: w.id, number, count: w.count }))];
  return { ...drafts, picked: drafts.picked.filter((p) => !(p.number === number && ids.has(p.id))), sent: sent.slice(-SENT_MAX) };
}

/**
 * The drafts without the ids picked for `threads.number` that GitHub no longer lists as unresolved threads or as notes, and without
 * that pull request's sent marks whose thread is gone, resolved or has another count. Entries of other pull requests are never judged
 * (drafts outlive a branch switch). An id the reply doesn't list at all stays while the reply is cut (`more_threads > 0` or
 * `more_notes`): it may be past the cut. The same object when nothing goes.
 */
export function pruned(drafts: ChangesDrafts | undefined, threads: PullThreads): ChangesDrafts | undefined {
  if (drafts === undefined) return undefined;
  const cutOff = threads.more_threads > 0 || threads.more_notes;
  const byId = new Map(threads.threads.map((t) => [t.id, t]));
  const notes = new Set(threads.notes.map((n) => n.id));
  const listed = (id: string) => byId.has(id) || notes.has(id);
  const keepPick = (p: PickedComment) => p.number !== threads.number || notes.has(p.id) || (byId.has(p.id) && !byId.get(p.id)!.resolved) || (cutOff && !listed(p.id));
  const keepSent = (s: SentComment) => {
    if (s.number !== threads.number) return true;
    const t = byId.get(s.id);
    if (t !== undefined) return !t.resolved && commentCount(t) === s.count;
    return notes.has(s.id) ? s.count === 1 : cutOff && !listed(s.id);
  };
  const picked = drafts.picked.filter(keepPick);
  const sent = drafts.sent.filter(keepSent);
  return picked.length === drafts.picked.length && sent.length === drafts.sent.length ? drafts : { ...drafts, picked, sent };
}
