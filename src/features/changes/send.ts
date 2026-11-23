import type { ChangesDrafts } from "../../generated/ChangesDrafts";
import type { CommentsReply } from "../../generated/CommentsReply";
import type { DraftComment } from "../../generated/DraftComment";
import type { ReviewReply } from "../../generated/ReviewReply";
import { addFailure, type Delivery } from "../regions/deliver";
import { type Refusal, refusalOf } from "./changesApi";
import { failureOf } from "./githubApi";

const comments = (n: number) => `${n} ${n === 1 ? "comment" : "comments"}`;

/** The sentence for a 409 from /comments (`send`) or /review: what was kept or not typed. */
export function refusalNotice(reason: Refusal, name: string, what: "send" | "review"): string {
  const end = what === "send" ? "so the comments are kept." : "so nothing was typed.";
  switch (reason) {
    case "waiting":
      return `${name} is waiting for an answer, ${end}`;
    case "ended":
      return `${name} has ended, ${end}`;
    case "paused":
      return `${name} is paused, ${end}`;
    case "elevated":
      return what === "send" ? `${name} runs elevated, so Marshell can't paste into it. The comments are kept.` : `${name} runs elevated, so nothing was typed.`;
    case "no_agent":
      return `${name} runs no agent, ${end}`;
    case "not_ready":
      return `${name} isn't ready yet, ${end}`;
    default: {
      const unreachable: never = reason;
      return unreachable;
    }
  }
}

/** The sentence for an ask of the agent that failed: a refusal in the review path's words, else the core's own sentence. */
export function askNotice(e: unknown, name: string): string {
  const reason = refusalOf(e);
  return reason === null ? failureOf(e).reason : refusalNotice(reason, name, "review");
}

/** What a send did: whether the comments left (and are cleared), what to announce, and whether the keyboard goes to the terminal. */
export type SendResult = { sent: boolean; notice: string; focus: boolean };

/** Whether the keyboard goes back to the list after a send: the comments went, the terminal didn't take the keyboard (a copy), and none are `left`, so the Send bar and its button are gone. */
export const backToList = (result: SendResult, left: number): boolean => result.sent && !result.focus && left === 0;

export type SendInput = {
  /** `deliveryFor` of the session; null when there is none. */
  delivery: Delivery | null;
  count: number;
  message: string;
  post(text: string): Promise<CommentsReply>;
  write(text: string): Promise<void>;
};

/** Hands the comments over: onto the clipboard for a session with no prompt line, else through the core. A refusal or failure keeps them. */
export async function runSend(o: SendInput): Promise<SendResult> {
  const kept = (notice: string): SendResult => ({ sent: false, notice, focus: false });
  const { delivery } = o;
  if (delivery === null) return kept("No session to send the comments to.");
  if ("refused" in delivery) return kept(delivery.refused);
  const { name } = delivery;
  if (delivery.via === "clipboard") {
    try {
      await o.write(o.message);
    } catch {
      return kept("Couldn't copy the comments.");
    }
    return { sent: true, notice: `Copied ${comments(o.count)}. ${name} has no prompt line.`, focus: false };
  }
  try {
    const reply = await o.post(o.message);
    const notice = reply.form === "file" ? `Pasted a link to ${comments(o.count)} into ${name}.` : `Pasted ${comments(o.count)} into ${name}.`;
    return { sent: true, notice, focus: true };
  } catch (e) {
    const reason = refusalOf(e);
    return kept(reason === null ? addFailure(e) : refusalNotice(reason, name, "send"));
  }
}

/**
 * The drafts once a send is over: the comments `sent` held are cleared when they left, and the folds stay. A comment added
 * while the send went, or one whose text changed meanwhile, wasn't in the message and stays. A kept send leaves the same object.
 */
export function afterSend(drafts: ChangesDrafts, result: SendResult, sent: readonly DraftComment[]): ChangesDrafts {
  if (!result.sent) return drafts;
  const went = new Map(sent.map((c) => [c.id, c.text]));
  return { ...drafts, comments: drafts.comments.filter((c) => went.get(c.id) !== c.text) };
}

export type ReviewResult = { done: boolean; notice: string };

export type ReviewInput = {
  name: string;
  base: string | null;
  /** The agent's review command (`reviewCommandOf`), null where it has none. */
  command: string | null;
  ask(base: string | null): Promise<ReviewReply>;
};

/** Types the review command or request into the session's prompt, never Enter. */
export async function runReview(o: ReviewInput): Promise<ReviewResult> {
  try {
    const { typed } = await o.ask(o.base);
    return typed === o.command
      ? { done: true, notice: `Typed ${typed} into ${o.name}. Press Enter there to start the review.` }
      : { done: true, notice: `Typed a review request into ${o.name}. Press Enter there to send it.` };
  } catch (e) {
    const reason = refusalOf(e);
    return { done: false, notice: reason === null ? addFailure(e) : refusalNotice(reason, o.name, "review") };
  }
}
