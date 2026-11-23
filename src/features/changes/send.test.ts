import { describe, expect, it, vi } from "vitest";
import type { ChangesDrafts } from "../../generated/ChangesDrafts";
import type { DraftComment } from "../../generated/DraftComment";
import { ApiError } from "../../lib/api/client";
import { afterSend, askNotice, refusalNotice, runReview, runSend } from "./send";

const c = (id: string): DraftComment => ({ id, path: "a.ts", side: "new", start: 1, end: 1, rows: 1, first: "f", hash: "h", quote: ["x"], text: "t" });
const drafts: ChangesDrafts = { checkout: "r", base: null, comments: [c("1"), c("2")], picked: [], sent: [], folds: { "a.ts": true } };
const paste = { name: "Claude", via: "paste" as const };
const reply = (form: "paste" | "file") => ({ form, path: null });
const conflict = (reason: string) => new ApiError(409, JSON.stringify({ reason }));

describe("runSend", () => {
  it("a refusal keeps the drafts", async () => {
    const r = await runSend({ delivery: paste, count: 2, message: "m", post: () => Promise.reject(conflict("waiting")), write: async () => {} });
    expect(r).toEqual({ sent: false, notice: "Claude is waiting for an answer, so the comments are kept.", focus: false });
    expect(afterSend(drafts, r, drafts.comments)).toBe(drafts);
  });
  it("a sent paste clears only the comments it held: one added or edited while it went stays", () => {
    const later = { ...drafts, comments: [c("1"), { ...c("2"), text: "edited" }, c("3")] };
    expect(afterSend(later, { sent: true, notice: "n", focus: true }, drafts.comments).comments).toEqual([{ ...c("2"), text: "edited" }, c("3")]);
  });
  it("a refused delivery is announced and keeps the comments", async () => {
    const post = vi.fn();
    const r = await runSend({ delivery: { name: "Claude", refused: "Claude is paused." }, count: 1, message: "m", post, write: async () => {} });
    expect(r).toMatchObject({ sent: false, notice: "Claude is paused." });
    expect(post).not.toHaveBeenCalled();
  });
  it("a send keeps the folds", async () => {
    const r = await runSend({ delivery: paste, count: 2, message: "m", post: async () => reply("paste"), write: async () => {} });
    expect(r).toEqual({ sent: true, notice: "Pasted 2 comments into Claude.", focus: true });
    expect(afterSend(drafts, r, drafts.comments)).toEqual({ ...drafts, comments: [] });
  });
  it("the message reaches postComments unchanged", async () => {
    const post = vi.fn(async (_text: string) => reply("file"));
    const r = await runSend({ delivery: paste, count: 1, message: "Comments:\n\n  a.ts:1 ", post, write: async () => {} });
    expect(post).toHaveBeenCalledWith("Comments:\n\n  a.ts:1 ");
    expect(r.notice).toBe("Pasted a link to 1 comment into Claude.");
  });
  it("the clipboard form copies and says the session has no prompt line", async () => {
    const write = vi.fn(async (_text: string) => {});
    const r = await runSend({ delivery: { name: "the shell", via: "clipboard" }, count: 3, message: "m", post: vi.fn(), write });
    expect(write).toHaveBeenCalledWith("m");
    expect(r).toEqual({ sent: true, notice: "Copied 3 comments. the shell has no prompt line.", focus: false });
  });
  it("another failure is announced as the failure", async () => {
    const r = await runSend({ delivery: paste, count: 1, message: "m", post: () => Promise.reject(new Error("boom")), write: async () => {} });
    expect(r).toEqual({ sent: false, notice: "boom", focus: false });
  });
});

describe("refusalNotice", () => {
  it("words each reason for Send and for Review", () => {
    expect(refusalNotice("ended", "Claude", "send")).toBe("Claude has ended, so the comments are kept.");
    expect(refusalNotice("not_ready", "Claude", "send")).toBe("Claude isn't ready yet, so the comments are kept.");
    expect(refusalNotice("not_ready", "Claude", "review")).toBe("Claude isn't ready yet, so nothing was typed.");
    expect(refusalNotice("paused", "Claude", "send")).toBe("Claude is paused, so the comments are kept.");
    expect(refusalNotice("elevated", "Claude", "send")).toBe("Claude runs elevated, so Marshell can't paste into it. The comments are kept.");
    expect(refusalNotice("no_agent", "the shell", "send")).toBe("the shell runs no agent, so the comments are kept.");
    expect(refusalNotice("waiting", "Claude", "review")).toBe("Claude is waiting for an answer, so nothing was typed.");
    expect(refusalNotice("elevated", "Claude", "review")).toBe("Claude runs elevated, so nothing was typed.");
    expect(refusalNotice("no_agent", "the shell", "review")).toBe("the shell runs no agent, so nothing was typed.");
  });
});

describe("runReview", () => {
  it("announces the command or the request", async () => {
    expect(await runReview({ name: "Claude", base: "origin/main", command: "/code-review", ask: async () => ({ typed: "/code-review" }) })).toEqual({
      done: true,
      notice: "Typed /code-review into Claude. Press Enter there to start the review.",
    });
    expect((await runReview({ name: "Claude", base: null, command: null, ask: async () => ({ typed: "Please review the changes." }) })).notice).toBe("Typed a review request into Claude. Press Enter there to send it.");
  });
  it("a refusal types nothing", async () => {
    expect(await runReview({ name: "Claude", base: null, command: null, ask: () => Promise.reject(conflict("ended")) })).toEqual({ done: false, notice: "Claude has ended, so nothing was typed." });
  });
});

describe("askNotice", () => {
  it("words a refusal as the review path does", () => {
    expect(askNotice(new ApiError(409, JSON.stringify({ reason: "elevated" })), "api")).toBe("api runs elevated, so nothing was typed.");
    expect(askNotice(new ApiError(409, JSON.stringify({ reason: "waiting" })), "api")).toBe("api is waiting for an answer, so nothing was typed.");
  });
  it("shows a 409's own sentence and any other failure's reason", () => {
    expect(askNotice(new ApiError(409, JSON.stringify({ reason: "The session moved to another folder. Look again before you go on." })), "api")).toBe("The session moved to another folder. Look again before you go on.");
    expect(askNotice(new Error("boom"), "api")).toBe("boom");
  });
});
