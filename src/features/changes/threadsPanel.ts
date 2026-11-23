import type { ChangesDrafts } from "../../generated/ChangesDrafts";
import type { PullThreads } from "../../generated/PullThreads";
import type { ReviewThread } from "../../generated/ReviewThread";
import type { Placement } from "./comments";
import type { Shown } from "./drafts";
import { githubCopy as gc } from "./githubCopy";
import type { Item, Section } from "./model";
import type { BarView } from "./pullModel";
import { threadCopy } from "./threadCopy";
import { type GithubItems, commentCount, placeThread, shownThreads, toResolve } from "./threads";
import { type PickedItem, addUnresolved, isSent, pickedItems } from "./threadsSend";

/** What the Send bar counts: the user's comments plus the picked threads and notes the reply still lists. */
export const sendCount = (drafts: ChangesDrafts | undefined, threads: PullThreads | null): number => (drafts?.comments.length ?? 0) + pickedItems(drafts, threads).length;

/** Whether the panel draws its list rather than an empty state: the diff is listed, and it has a file or GitHub has a thread, an open note or a line to show. */
export function hasList(listed: unknown, sections: Section[], github: GithubItems | undefined): boolean {
  if (listed === null || listed === undefined) return false;
  return sections.length > 0 || (github !== undefined && (github.threads.length > 0 || github.notes.length > 0 || github.lines.length > 0));
}

/** A send's notice, with the sentence for the picked comments that didn't fit when some were left. */
export const sendNotice = (notice: string, left: number): string => (left > 0 ? `${notice} ${threadCopy.leftOver(left)}` : notice);

/** The strip's line while the comments of pull request `number` can't be read; null without a failure or a pull request. */
export const readNotice = (number: number | null, failed: string | null): string | null => (number === null || failed === null ? null : threadCopy.readFailed(number, failed));

const unresolved = (t: ReviewThread) => !t.resolved && !t.outdated;

/**
 * The next thread to resolve after the item at `focus`, wrapping; null with none. A thread in the list counts at its place; one
 * in `open` (the unresolved threads of the reply) that the list doesn't draw, its file being folded, counts just after its file's header.
 */
export function nextToResolve(items: Item[], focus: number, open: ReviewThread[] = []): ReviewThread | null {
  const spots: { at: number; thread: ReviewThread }[] = [];
  const drawn = new Set<string>();
  items.forEach((item, at) => {
    if (item.kind !== "thread" || !unresolved(item.thread)) return;
    spots.push({ at, thread: item.thread });
    drawn.add(item.thread.id);
  });
  for (const t of open) {
    if (drawn.has(t.id) || !unresolved(t)) continue;
    const header = items.findIndex((i) => i.kind === "file" && i.file.path === t.path);
    if (header >= 0) spots.push({ at: header + 0.5, thread: t });
  }
  spots.sort((a, b) => a.at - b.at);
  return (spots.find((s) => s.at > focus) ?? spots[0])?.thread ?? null;
}

/** Each thread's place in the diff, by id: `placeThread` against the file of its path. */
export function placementsOf(threads: ReviewThread[], sections: Section[]): Map<string, Placement | null> {
  const byPath = new Map(sections.map((s) => [s.path, s]));
  return new Map(threads.map((t) => [t.id, placeThread(t, byPath.get(t.path))]));
}

/**
 * The picked threads in the diff's order, then the notes: threads of a file in `sections` by the file's place and then by their
 * first line (the placement's, else the thread's own); then threads with no file in the diff; ties keep the given order.
 */
export function inDiffOrder(items: PickedItem[], sections: Section[], placements: Map<string, Placement | null>): PickedItem[] {
  const fileAt = new Map(sections.map((s, i) => [s.path, i]));
  const threads = items.filter((i): i is Extract<PickedItem, { kind: "thread" }> => i.kind === "thread");
  const line = (i: Extract<PickedItem, { kind: "thread" }>) => placements.get(i.thread.id)?.start ?? i.thread.start ?? i.thread.original_start ?? 0;
  const inDiff = threads.filter((i) => fileAt.has(i.thread.path)).sort((a, b) => fileAt.get(a.thread.path)! - fileAt.get(b.thread.path)! || line(a) - line(b));
  return [...inDiff, ...threads.filter((i) => !fileAt.has(i.thread.path)), ...items.filter((i) => i.kind === "talk")];
}

/** What the list holds of GitHub's comments: the threads shown, where each sits, the notes when the list is open, and how each id stands in the send. Undefined with no reply. */
export function githubItemsOf(threads: PullThreads | null, drafts: ChangesDrafts | undefined, o: { showOld: boolean; talkOpen: boolean; sections: Section[]; agent: string | null }): GithubItems | undefined {
  if (threads === null) return undefined;
  // The threads the switch shows, and every thread picked for this pull request (so what the Send bar counts and sends is always drawn), in the reply's order.
  const showing = new Set(shownThreads(threads.threads, o.showOld).map((t) => t.id));
  const pickedIds = new Set((drafts?.picked ?? []).filter((p) => p.number === threads.number).map((p) => p.id));
  const shown = threads.threads.filter((t) => showing.has(t.id) || pickedIds.has(t.id));
  const counts = new Map<string, number>([...threads.threads.map((t): [string, number] => [t.id, commentCount(t)]), ...threads.notes.map((n): [string, number] => [n.id, 1])]);
  const picks = new Map((drafts?.picked ?? []).filter((p) => p.number === threads.number).map((p) => [p.id, p.note]));
  const sent = (drafts?.sent ?? []).filter((s) => s.number === threads.number);
  return {
    threads: shown,
    placements: placementsOf(shown, o.sections),
    notes: o.talkOpen ? threads.notes : [],
    lines: [...(threads.more_threads > 0 ? [threadCopy.moreThreads(threads.more_threads)] : []), ...(o.talkOpen && threads.more_notes ? [threadCopy.moreNotes] : [])],
    state: (id) => ({ picked: picks.has(id), note: picks.get(id) ?? "", sent: isSent(id, counts.get(id) ?? 0, sent) }),
    agent: o.agent,
  };
}

/** "Add all unresolved": the drafts with the unresolved threads picked (null when nothing was added) and what to say. */
export function addAllOf(drafts: ChangesDrafts | undefined, shown: Shown, threads: PullThreads): { drafts: ChangesDrafts | null; say: string } {
  const r = addUnresolved(drafts, shown, threads);
  const left = toResolve(threads.threads).filter((t) => !r.drafts.picked.some((p) => p.id === t.id) && !isSent(t.id, commentCount(t), r.drafts.sent.filter((s) => s.number === threads.number))).length;
  if (r.added === 0) return { drafts: null, say: left > 0 ? threadCopy.tooMany : threadCopy.nothingToAdd };
  const say = threadCopy.added(r.added, sendCount(r.drafts, threads));
  return { drafts: r.drafts, say: left > 0 ? `${say} ${threadCopy.tooMany}` : say };
}

/** The drafts after "Discard all": the comments and the picks of pull request `number` go; the sent marks and other pull requests' picks stay. */
export const discarded = (drafts: ChangesDrafts, number: number | null): ChangesDrafts => ({ ...drafts, comments: [], picked: drafts.picked.filter((p) => p.number !== number) });

/** Why a command about a pull request's comments can't run on this bar, as `commandOf` would say it; null for the bar of a pull request. `off` is the setting being off. */
export function commentSay(view: BarView | null, off: boolean): string | null {
  if (view === null) return off ? gc.pullsOff : gc.unavailable;
  if (view.scope === "pull") return null;
  return view.scope === "branch" ? gc.noPullHere : view.line;
}
