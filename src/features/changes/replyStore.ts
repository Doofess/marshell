import type { ReplyState } from "./boxModel";

/** One session's reply state, held outside the panel so a remount keeps it. */
export type ReplyHolder = { current: ReplyState; listeners: Set<() => void> };

const held = new Map<string, ReplyHolder>();

const idle = (h: ReplyHolder): boolean => h.listeners.size === 0 && h.current.box === null && h.current.texts.size === 0 && h.current.out.size === 0;

/** The holder for `session`, made on first use. */
export function replyHolder(session: string): ReplyHolder {
  let h = held.get(session);
  if (h === undefined) {
    h = { current: { box: null, texts: new Map(), out: new Set() }, listeners: new Set() };
    held.set(session, h);
  }
  return h;
}

/** Sets the holder's state and tells every listener; a session with nothing left to hold (no box, no kept text, no reply out) and no listener is forgotten. */
export function setHeld(session: string, next: ReplyState): void {
  const h = replyHolder(session);
  h.current = next;
  for (const fn of [...h.listeners]) fn();
  if (idle(h)) held.delete(session);
}

/** Listens for changes; returns the function that stops listening (and forgets an empty session with no listener left). */
export function listen(session: string, fn: () => void): () => void {
  const h = replyHolder(session);
  h.listeners.add(fn);
  return () => {
    h.listeners.delete(fn);
    if (idle(h) && held.get(session) === h) held.delete(session);
  };
}

/** The sessions that hold something now (for tests). */
export const heldSessions = (): string[] => [...held.keys()];
