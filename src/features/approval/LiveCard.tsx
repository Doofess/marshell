import { useCallback, useEffect, useReducer, useRef, useState, type ReactNode } from "react";
import { ApproveCard } from "./ApproveCard";
import type { CardAction } from "./arming";
import type { ApprovalRequest } from "./types";
import { useArming } from "./useArming";
import { useHold } from "./useHold";
import { receiptText, subject } from "./headline";

/** How long a receipt stays, and how long its exit takes. */
export const RECEIPT_MS = 1500;
export const EXIT_MS = 140;
export const GONE_MESSAGE = "This request is no longer waiting. Answer it in the terminal.";
export const UNREACHABLE_MESSAGE = "Couldn't reach the core. Answer it in the terminal.";

/** What the person chose. "terminal" is "Answer in terminal": the agent asks in its own prompt. */
export type Verdict = "allowed" | "denied" | "terminal";
/** What a card does, as the core is told: allow once, deny, always allow in this project, or hand it back to the terminal. */
export type Answer = "allow" | "deny" | "always" | "terminal";

export type CardMachine =
  | { phase: "pending"; notice?: string }
  | { phase: "sending"; verdict: Verdict }
  | { phase: "receipt"; verdict: Verdict }
  | { phase: "gone"; message: string }
  /** `from` is what was on screen as the card leaves. */
  | { phase: "exiting"; from: CardMachine }
  | { phase: "removed" };

export type CardEvent =
  | { type: "act"; verdict: Verdict }
  | { type: "ok" }
  | { type: "gone" }
  | { type: "conflict"; text: string }
  | { type: "timer" }
  /** The request left the list without our answer: answered in the terminal, released, or its tab closed. */
  | { type: "vanished" };

/** One card's life: pending, sending its answer, a receipt for 1500 ms, a 140 ms exit, then gone. Pure. */
export function cardReducer(m: CardMachine, e: CardEvent): CardMachine {
  switch (m.phase) {
    case "pending":
      if (e.type === "act") return { phase: "sending", verdict: e.verdict };
      if (e.type === "vanished") return { phase: "exiting", from: m };
      return m;
    case "sending":
      // The frame that drops the request can beat the answer to us: keep waiting for the answer.
      if (e.type === "ok") return { phase: "receipt", verdict: m.verdict };
      if (e.type === "gone") return { phase: "gone", message: GONE_MESSAGE };
      if (e.type === "conflict") return { phase: "pending", notice: e.text };
      return m;
    case "receipt":
    case "gone":
      return e.type === "timer" ? { phase: "exiting", from: m } : m;
    case "exiting":
      return e.type === "timer" ? { phase: "removed" } : m;
    case "removed":
      return m;
  }
}

/** How long a card stays in a phase before its timer event, or undefined for a phase that waits for something else. */
export const timerFor = (m: CardMachine): number | undefined => (m.phase === "receipt" || m.phase === "gone" ? RECEIPT_MS : m.phase === "exiting" ? EXIT_MS : undefined);

/** The request as the lane draws it in a machine state. */
export function displayRequest(r: ApprovalRequest, m: CardMachine): ApprovalRequest {
  switch (m.phase) {
    case "pending":
      return { ...r, state: m.notice ? { phase: "pending", notice: m.notice } : { phase: "pending" } };
    case "sending":
      return { ...r, state: { phase: "pending" }, busy: true };
    case "receipt":
      return { ...r, state: m.verdict === "terminal" ? { phase: "terminal" } : { phase: "decided", verdict: m.verdict } };
    case "gone":
      return { ...r, state: { phase: "gone", message: m.message } };
    case "exiting": {
      if (m.from.phase === "pending") {
        return { ...r, state: r.state.phase === "released" ? r.state : { phase: "gone", message: `No longer waiting · ${subject(r.detail)}` }, leaving: true };
      }
      return { ...displayRequest(r, m.from), leaving: true };
    }
    case "removed":
      return { ...r, state: { phase: "gone", message: `No longer waiting · ${subject(r.detail)}` }, leaving: true };
  }
}

type Tracked = { request: ApprovalRequest; m: CardMachine };
type Action = { type: "sync"; entries: ApprovalRequest[] } | { type: "event"; id: string; e: CardEvent };

function reduceCards(cards: Tracked[], a: Action): Tracked[] {
  if (a.type === "event") {
    let changed = false;
    const next = cards.map((c) => {
      if (c.request.id !== a.id) return c;
      const m = cardReducer(c.m, a.e);
      if (m === c.m) return c;
      changed = true;
      return { ...c, m };
    });
    return changed ? next : cards;
  }
  const byId = new Map(cards.map((c) => [c.request.id, c]));
  const present = new Set(a.entries.map((e) => e.id));
  const next: Tracked[] = a.entries.map((e) => {
    const c = byId.get(e.id);
    // While the answer is under way or showing, the card keeps the request it was answering.
    if (!c) return { request: e, m: { phase: "pending" } };
    return c.m.phase === "pending" ? { request: e, m: c.m } : c;
  });
  for (const c of cards) {
    if (present.has(c.request.id)) continue;
    // A one-liner has no card to animate; a card that is already removed has nothing left to show.
    if (c.request.terminalOnly || c.m.phase === "removed") continue;
    next.push({ ...c, m: cardReducer(c.m, { type: "vanished" }) });
  }
  return next;
}

/**
 * Every lane entry's card life. The returned requests are what the lane draws: waiting entries, plus the receipts and exits that
 * outlast the entry itself (the core drops a request the moment it is answered). The needs-you count stays the entries.
 */
export function useCards(entries: ApprovalRequest[], onNotice?: (text: string) => void): {
  requests: ApprovalRequest[];
  /** Runs the call once for a waiting card, and moves the card through sending to its receipt. */
  answer: (id: string, verdict: Verdict, call: () => Promise<"ok" | "gone" | { conflict: string }>) => void;
} {
  const [cards, dispatch] = useReducer(reduceCards, []);
  const latest = useRef(cards);
  latest.current = cards;
  const sending = useRef(new Set<string>());
  const notice = useRef(onNotice);
  notice.current = onNotice;
  useEffect(() => dispatch({ type: "sync", entries }), [entries]);

  const timers = useRef(new Map<string, { phase: string; handle: ReturnType<typeof setTimeout> }>());
  useEffect(() => {
    const live = new Set<string>();
    for (const c of cards) {
      const id = c.request.id;
      const ms = timerFor(c.m);
      if (ms === undefined) continue;
      live.add(id);
      const t = timers.current.get(id);
      if (t?.phase === c.m.phase) continue;
      if (t) clearTimeout(t.handle);
      timers.current.set(id, { phase: c.m.phase, handle: setTimeout(() => dispatch({ type: "event", id, e: { type: "timer" } }), ms) });
    }
    for (const [id, t] of timers.current) {
      if (live.has(id)) continue;
      clearTimeout(t.handle);
      timers.current.delete(id);
    }
  }, [cards]);
  useEffect(() => {
    const all = timers.current;
    return () => all.forEach((t) => clearTimeout(t.handle));
  }, []);

  const answer = useCallback((id: string, verdict: Verdict, call: () => Promise<"ok" | "gone" | { conflict: string }>) => {
    const card = latest.current.find((c) => c.request.id === id);
    if (!card || card.m.phase !== "pending" || sending.current.has(id)) return;
    sending.current.add(id);
    dispatch({ type: "event", id, e: { type: "act", verdict } });
    call().then(
      (r) => {
        dispatch({ type: "event", id, e: r === "ok" ? { type: "ok" } : r === "gone" ? { type: "gone" } : { type: "conflict", text: r.conflict } });
        notice.current?.(r === "gone" ? GONE_MESSAGE : r !== "ok" ? r.conflict : verdict === "terminal" ? `Answered in terminal · ${subject(card.request.detail)}` : receiptText(card.request.detail, verdict));
      },
      () => {
        dispatch({ type: "event", id, e: { type: "conflict", text: UNREACHABLE_MESSAGE } });
        notice.current?.(UNREACHABLE_MESSAGE);
      },
    ).finally(() => sending.current.delete(id));
  }, []);

  return { requests: cards.filter((c) => c.m.phase !== "removed").map((c) => displayRequest(c.request, c.m)), answer };
}

export type LiveCardProps = {
  request: ApprovalRequest;
  /** In the drawer: the payload is shown in full from the start. */
  inDrawer?: boolean;
  onAnswer: (request: ApprovalRequest, answer: Answer) => void;
  onJump: (request: ApprovalRequest) => void;
  /** Space or "Show all" on a card in the lane: the drawer opens with the full request. */
  onExpand: (request: ApprovalRequest) => void;
  /** Called when the card that had the focus answers or leaves, so the focus is not lost to the page. */
  onRestoreFocus: () => void;
};

/**
 * The approve card wired to the app: it owns the 500 ms arming (re-armed on focus and when the request changes) and the 600 ms hold
 * for a risky Allow, and turns the card's actions into answers. Enter jumps to the session and never allows.
 */
export function LiveCard({ request, inDrawer = false, onAnswer, onJump, onExpand, onRestoreFocus }: LiveCardProps): ReactNode {
  const busy = request.busy === true;
  const [expanded, setExpanded] = useState(inDrawer);
  const { armed, rearm } = useArming(`${request.id}\n${JSON.stringify(request.detail)}\n${request.alwaysRule ?? ""}`);
  const hadFocus = useRef(false);
  const element = useRef<HTMLDivElement>(null);
  const restore = useRef(onRestoreFocus);
  restore.current = onRestoreFocus;
  useEffect(
    () => () => {
      if (hadFocus.current) restore.current();
    },
    [],
  );
  const reply = (a: Answer) => {
    if (hadFocus.current) {
      hadFocus.current = false;
      restore.current();
    }
    onAnswer(request, a);
  };
  const canAnswer = armed && !busy && request.state.phase === "pending";
  const hold = useHold(() => reply("allow"), canAnswer, () => element.current?.contains(document.activeElement) === true);
  const onAction = (a: CardAction | `answer:${number}`) => {
    switch (a) {
      case "allow":
      case "deny":
      case "always":
      case "terminal":
        reply(a);
        break;
      case "hold-allow":
        hold.begin();
        break;
      case "expand":
        setExpanded(true);
        if (!inDrawer) onExpand(request);
        break;
      case "jump":
        onJump(request);
        break;
    }
  };
  return (
    <div ref={element} className="live-card" onFocus={() => (hadFocus.current = true)} onBlur={(e) => {
      if (e.currentTarget.contains(e.relatedTarget)) return;
      hadFocus.current = false;
      hold.cancel();
    }}>
      <ApproveCard request={request} expanded={expanded} armed={canAnswer} holdProgress={hold.progress} onAction={onAction} onHoldStart={hold.begin} onHoldEnd={hold.cancel} onFocus={rearm} />
    </div>
  );
}
