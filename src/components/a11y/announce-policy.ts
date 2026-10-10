/**
 * The app's one voice for screen readers. Every feature says things through this module; nothing else in the app is a live
 * region (src/components/a11y/sharedBlocks.test.ts fails the build if one appears). The rules are the policy in
 * docs/PLAN.md, "Accessibility safety net": silent on first render, never repeat the same words, at most one message per 1.5 s with
 * the latest one winning, and only what needs the user.
 */
export type AnnouncePriority = "polite" | "assertive";
export type Message = { text: string; priority: AnnouncePriority; seq: number };

/**
 * The user's setting. Only "needs-you" is built (the default). "everything" (every status change in every visible pane) and
 * "on-request" (nothing until the announce-status command) are typed so the setting page can offer them later; until then
 * "everything" behaves like "needs-you" and "on-request" is silent apart from the command.
 */
export type AnnounceMode = "needs-you" | "everything" | "on-request";

export const ANNOUNCE_INTERVAL_MS = 1500;

/** What happened, in terms of whether the user needs to hear about it. */
export type AnnounceKind =
  | "request" // a permission request arrived for the focused session
  | "question" // the focused session asked a question
  | "stopped" // the focused session stopped with an error
  | "ended" // the focused session ended
  | "switched" // focus moved to another session
  | "none-selected" // nothing is selected
  | "on-request" // the user asked ("Read the focused session's status")
  | "progress" // working, tool use, subagents: never announced
  | "working"
  | "done"
  | "other-pane"; // anything in a pane the user is not in

const NEEDS_YOU: ReadonlySet<AnnounceKind> = new Set(["request", "question", "stopped", "ended", "switched", "none-selected", "on-request"]);

export function shouldAnnounce(kind: AnnounceKind, mode: AnnounceMode): boolean {
  if (mode === "on-request") return kind === "on-request";
  return NEEDS_YOU.has(kind);
}

export type Announcer = {
  /** `repeat` is for something the user asked to hear: it is said even if it is what was just said. */
  say(text: string, opts?: { priority?: AnnouncePriority; repeat?: boolean }): void;
  /** Says the held message if the interval is over. The component calls it from a timer set for `pendingDelay()`. */
  flush(): void;
  /** Milliseconds until the held message may be said, or null when nothing is held. */
  pendingDelay(): number | null;
  current(): Message;
  subscribe(listener: (m: Message) => void): () => void;
};

export function createAnnouncer({ now = () => Date.now() }: { now?: () => number } = {}): Announcer {
  let current: Message = { text: "", priority: "polite", seq: 0 };
  let lastSpokenAt = Number.NEGATIVE_INFINITY;
  let held: { text: string; priority: AnnouncePriority; repeat: boolean } | null = null;
  const listeners = new Set<(m: Message) => void>();

  const speak = (text: string, priority: AnnouncePriority, repeat = false) => {
    held = null;
    if (!repeat && text === current.text && current.seq > 0) return; // never the same words twice running
    current = { text, priority, seq: current.seq + 1 };
    lastSpokenAt = now();
    listeners.forEach((l) => l(current));
  };

  return {
    say(raw, opts) {
      const text = raw.trim();
      if (!text) return;
      const priority = opts?.priority ?? "polite";
      const repeat = opts?.repeat ?? false;
      if (priority === "assertive" || now() - lastSpokenAt >= ANNOUNCE_INTERVAL_MS) speak(text, priority, repeat);
      else held = { text, priority, repeat }; // within the interval: the latest message wins
    },
    flush() {
      if (held && now() - lastSpokenAt >= ANNOUNCE_INTERVAL_MS) speak(held.text, held.priority, held.repeat);
    },
    pendingDelay() {
      return held ? Math.max(0, ANNOUNCE_INTERVAL_MS - (now() - lastSpokenAt)) : null;
    },
    current: () => current,
    subscribe(l) {
      listeners.add(l);
      return () => listeners.delete(l);
    },
  };
}
