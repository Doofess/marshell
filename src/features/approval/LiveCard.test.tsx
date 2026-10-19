import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { APPROVALS } from "./fixtures";
import { GONE_MESSAGE, RECEIPT_MS, EXIT_MS, cardReducer, displayRequest, type CardEvent, type CardMachine } from "./LiveCard";
import { ApproveCard } from "./ApproveCard";

const run = (events: CardEvent[], from: CardMachine = { phase: "pending" }) => events.reduce(cardReducer, from);

describe("cardReducer", () => {
  it("act then ok gives a receipt, then exiting on the first timer, then removed on the second", () => {
    const sending = run([{ type: "act", verdict: "allowed" }]);
    expect(sending).toEqual({ phase: "sending", verdict: "allowed" });
    const receipt = run([{ type: "ok" }], sending);
    expect(receipt).toEqual({ phase: "receipt", verdict: "allowed" });
    const exiting = run([{ type: "timer" }], receipt);
    expect(exiting.phase).toBe("exiting");
    expect(run([{ type: "timer" }], exiting)).toEqual({ phase: "removed" });
  });
  it("times the receipt 1500 ms and the exit 140 ms", () => {
    expect(RECEIPT_MS).toBe(1500);
    expect(EXIT_MS).toBe(140);
  });
  it("gone gives the gone message, which then leaves the same way", () => {
    const gone = run([{ type: "act", verdict: "denied" }, { type: "gone" }]);
    expect(gone).toEqual({ phase: "gone", message: GONE_MESSAGE });
    expect(GONE_MESSAGE).toBe("This request is no longer waiting. Answer it in the terminal.");
    expect(run([{ type: "timer" }, { type: "timer" }], gone)).toEqual({ phase: "removed" });
  });
  it("vanished while pending gives exiting, then removed", () => {
    const exiting = run([{ type: "vanished" }]);
    expect(exiting.phase).toBe("exiting");
    expect(run([{ type: "timer" }], exiting)).toEqual({ phase: "removed" });
  });
  it("keeps waiting for the answer when the request leaves while it is being sent", () => {
    expect(run([{ type: "act", verdict: "allowed" }, { type: "vanished" }])).toEqual({ phase: "sending", verdict: "allowed" });
  });
  it("answers once: a second act while sending or after the receipt does nothing", () => {
    const sending = run([{ type: "act", verdict: "allowed" }]);
    expect(run([{ type: "act", verdict: "denied" }], sending)).toBe(sending);
    const receipt = run([{ type: "ok" }], sending);
    expect(run([{ type: "act", verdict: "denied" }], receipt)).toBe(receipt);
  });
  it("a conflict puts the card back with the core's text, still answerable", () => {
    const m = run([{ type: "act", verdict: "allowed" }, { type: "conflict", text: "no rule to add" }]);
    expect(m).toEqual({ phase: "pending", notice: "no rule to add" });
    expect(run([{ type: "act", verdict: "denied" }], m).phase).toBe("sending");
  });
});

describe("displayRequest", () => {
  const base = APPROVALS.safeBash;
  it("shows a sent answer as inert, a receipt as its verdict and a gone request as its message", () => {
    expect(displayRequest(base, { phase: "sending", verdict: "allowed" })).toMatchObject({ busy: true, state: { phase: "pending" } });
    expect(displayRequest(base, { phase: "receipt", verdict: "allowed" }).state).toEqual({ phase: "decided", verdict: "allowed" });
    expect(displayRequest(base, { phase: "receipt", verdict: "terminal" }).state).toEqual({ phase: "terminal" });
    expect(displayRequest(base, { phase: "gone", message: GONE_MESSAGE }).state).toEqual({ phase: "gone", message: GONE_MESSAGE });
    expect(displayRequest(base, { phase: "exiting", from: { phase: "receipt", verdict: "denied" } })).toMatchObject({ leaving: true, state: { phase: "decided", verdict: "denied" } });
  });
});

describe("receipts", () => {
  it("say what was answered, and the gone message", () => {
    const allowed = renderToStaticMarkup(<ApproveCard request={displayRequest(APPROVALS.safeBash, { phase: "receipt", verdict: "allowed" })} />);
    expect(allowed).toContain("Allowed · npm test");
    const denied = renderToStaticMarkup(<ApproveCard request={displayRequest(APPROVALS.safeBash, { phase: "receipt", verdict: "denied" })} />);
    expect(denied).toContain("Denied · npm test");
    const gone = renderToStaticMarkup(<ApproveCard request={displayRequest(APPROVALS.safeBash, { phase: "gone", message: GONE_MESSAGE })} />);
    expect(gone).toContain("This request is no longer waiting. Answer it in the terminal.");
    expect(gone).toContain('role="status"');
  });
  it("shows the core's refusal on a card that is still waiting", () => {
    const h = renderToStaticMarkup(<ApproveCard request={displayRequest(APPROVALS.safeBash, { phase: "pending", notice: "no rule to add" })} />);
    expect(h).toContain("no rule to add");
    expect(h).toContain('data-phase="pending"');
  });
});

describe("the card's wait", () => {
  it("reads <1m for a request waiting under a minute", () => {
    const h = renderToStaticMarkup(<ApproveCard request={{ ...APPROVALS.safeBash, waitingMs: 12_000 }} />);
    expect(h).toContain("&lt;1m");
    expect(h).not.toContain("12s");
  });
});
