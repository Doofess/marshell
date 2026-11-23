import type { DraftComment } from "../../generated/DraftComment";
import { place } from "./comments";
import type { Item, Section } from "./model";

export type BoxAnchor = Omit<DraftComment, "id" | "text">;

/** The comment box: the rows it is about and, when it edits a comment, that comment's id. */
export type BoxState = { anchor: BoxAnchor; id: string | null };

/** The box's React key: path, side, start and end. A reply that moves lines never changes it, so the box keeps its text and caret. */
export const anchorKey = (a: BoxAnchor): string => `${a.path}\u0000${a.side}\u0000${a.start}\u0000${a.end}`;

/** The box open on a GitHub thread: a reply to it, or the user's note on it. */
export type ThreadBox = { kind: "reply" | "note"; thread: string; text: string; error: string | null };

/** The reply boxes' state: the open box, the text kept per thread, and the threads with a reply out. */
export type ReplyState = { box: ThreadBox | null; texts: Map<string, string>; out: Set<string> };

/** The index of the item a thread's box is drawn under (the thread's or the note's own item); -1 when it left the list. */
export const threadBoxAfter = (items: Item[], box: ThreadBox): number => items.findIndex((i) => (i.kind === "thread" || i.kind === "talk") && i.key.slice(i.key.indexOf(":") + 1) === box.thread);

/** Whether a reply to `thread` may start: not while one is out for it, whether or not its box is open. */
export const canPost = (state: ReplyState, thread: string): boolean => !state.out.has(thread);

/** The check and the mark of a post in one call: null when a reply to `thread` is out; else the state with it out and, when the open box is that thread's reply box, its error cleared. The panel sets the result in the same tick, before the request starts. */
export function beginPost(state: ReplyState, thread: string): ReplyState | null {
  if (!canPost(state, thread)) return null;
  const b = state.box;
  const box = b !== null && b.kind === "reply" && b.thread === thread ? { ...b, error: null } : b;
  return { ...state, box, out: new Set(state.out).add(thread) };
}

/** What an answer does: the thread leaves `out` either way. A post closes its reply box and forgets the thread's kept text; a refusal keeps the box (when it is that reply's) with its text and the sentence. A box on another thread, or a note box, is left alone. */
export function afterReply(state: ReplyState, thread: string, result: { ok: true } | { ok: false; reason: string }): ReplyState {
  const out = new Set(state.out);
  out.delete(thread);
  const mine = state.box !== null && state.box.kind === "reply" && state.box.thread === thread;
  if (result.ok) {
    const texts = new Map(state.texts);
    texts.delete(thread);
    return { box: mine ? null : state.box, texts, out };
  }
  return { box: mine ? { ...state.box!, error: result.reason } : state.box, texts: state.texts, out };
}

/** The state with the open box closed: a reply's text is kept for its thread (forgotten when blank). */
export function closedThreadBox(state: ReplyState): ReplyState {
  const { box } = state;
  if (box === null) return state;
  if (box.kind !== "reply") return { ...state, box: null };
  const texts = new Map(state.texts);
  if (box.text.trim() === "") texts.delete(box.thread);
  else texts.set(box.thread, box.text);
  return { ...state, box: null, texts };
}

/** The state with a box open on `thread`, the one before it closed: a reply starts from the text kept for the thread, a note from the thread's own. */
export function openThreadBox(state: ReplyState, kind: ThreadBox["kind"], thread: string, note: string): ReplyState {
  const closed = closedThreadBox(state);
  return { ...closed, box: { kind, thread, text: kind === "reply" ? (closed.texts.get(thread) ?? "") : note, error: null } };
}

/** The index of the item the box is drawn under: the comment it edits, else the range's last row as `place` finds it now; -1 when the file is gone. */
export function boxAfter(items: Item[], sections: Section[], box: BoxState): number {
  if (box.id !== null) {
    const at = items.findIndex((i) => i.key === `comment:${box.id}`);
    if (at >= 0) return at;
  }
  const file = sections.find((s) => s.path === box.anchor.path);
  if (file === undefined) return -1;
  const placed = place({ ...box.anchor, id: "", text: "" }, file);
  if (placed.at !== null) return items.findIndex((i) => i.kind === "row" && i.file.path === file.path && i.at === placed.at);
  // Above the first row: under the file's header (and its note).
  const head = items.findIndex((i) => i.kind === "file" && i.file.path === file.path);
  return head >= 0 && items[head + 1]?.kind === "note" ? head + 1 : head;
}
