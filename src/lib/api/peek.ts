import type { Endpoint } from "../../generated/Endpoint";
import { coreFetch } from "./client";

/** The last `lines` rendered rows of a session's screen as plain text (the core clamps to 1..=100). Rejects when the core cannot read it. */
export async function peek(ep: Endpoint, tab: string, lines = 30): Promise<string[]> {
  const res = await coreFetch(ep, "GET", `/v1/sessions/${encodeURIComponent(tab)}/peek?lines=${lines}`);
  if (!res.ok) throw new Error(`The core answered ${res.status}.`);
  return ((await res.json()) as { rows: string[] }).rows;
}
