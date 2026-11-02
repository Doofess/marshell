import type { Endpoint } from "../../generated/Endpoint";
import type { PermissionDecision } from "../../generated/PermissionDecision";
import { coreFetch } from "./client";
import { decide as decideInWindow, type Invoke } from "./elevated";

const route = (rid: string, leaf: "decision" | "release") => `/v1/permissions/${encodeURIComponent(rid)}/${leaf}`;

/** The core refused the answer (409) and the request is still waiting; `conflict` is its sentence. */
export type DecideResult = "ok" | "gone" | { conflict: string };

/**
 * Answers a held request. 404 means it is no longer waiting (the hold expired, typing released it, or the tab closed) and 409
 * that the core could not build the answer (a scope with no rule to add). Anything else rejects.
 */
export async function decide(ep: Endpoint, rid: string, decision: PermissionDecision): Promise<DecideResult> {
  const res = await coreFetch(ep, "POST", route(rid, "decision"), decision);
  if (res.ok) return "ok";
  if (res.status === 404) return "gone";
  if (res.status === 409) return { conflict: await res.text() };
  throw new Error(`The core answered ${res.status}.`);
}

/**
 * `decide` for the row the request belongs to. An administrator row's answer goes through the window's own path (the token API
 * refuses it); the core's refusal comes back as `conflict`, in its words, and "no such request" as `gone`.
 */
export async function decideAs(ep: Endpoint, rid: string, decision: PermissionDecision, elevated: boolean, ask?: Invoke): Promise<DecideResult> {
  if (!elevated) return decide(ep, rid, decision);
  try {
    await decideInWindow(rid, decision, ask);
    return "ok";
  } catch (e) {
    const text = e instanceof Error ? e.message : String(e);
    return text === "no such request" ? "gone" : { conflict: text };
  }
}

/** Lets the agent ask in its own terminal instead. */
export async function release(ep: Endpoint, rid: string): Promise<"ok" | "gone"> {
  const res = await coreFetch(ep, "POST", route(rid, "release"));
  if (res.ok) return "ok";
  if (res.status === 404) return "gone";
  throw new Error(`The core answered ${res.status}.`);
}
