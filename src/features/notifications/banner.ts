import type { Os } from "../../lib/keymap";

export type { Os };

/** The things the plan says may ask for the user's eyes. A stuck session and a finished subagent never do. */
export type NotifyKind = "needs-permission" | "needs-question" | "done" | "error" | "stuck" | "subagent-done";

export type NotifyEvent = {
  kind: NotifyKind;
  number: number;
  name: string;
  project?: string;
  detail?: string;
  risky?: boolean;
  windowFocused?: boolean;
  backgroundMs?: number;
  resetsAt?: string;
  waitedMs?: number;
};

/**
 * What an OS banner says and promises. Marshell plays its own sound, so every banner is silent; none ever raises the
 * window; a click only focuses the session; and actions are only what the OS can really offer.
 */
export type Banner = {
  title: string;
  body: string;
  meta: string;
  actions: string[];
  silent: true;
  raisesWindow: false;
  activation: "focus-session";
  sessionNumber: number;
};

/** "Done" banners are only for sessions that have been in the background for more than 30 seconds. */
export const DONE_MIN_BACKGROUND_MS = 30_000;
/** Requests that arrive within this window fold into one banner. */
export const BATCH_WINDOW_MS = 3_000;

const base = (e: NotifyEvent) => ({
  silent: true as const,
  raisesWindow: false as const,
  activation: "focus-session" as const,
  sessionNumber: e.number,
  meta: `Session ${e.number}${e.project ? ` · ${e.project}` : ""}`,
});

export function bannerFor(e: NotifyEvent, os: Os): Banner | null {
  if (e.windowFocused) return null;
  switch (e.kind) {
    case "stuck":
    case "subagent-done":
      return null;
    case "done":
      return (e.backgroundMs ?? 0) > DONE_MIN_BACKGROUND_MS ? { ...base(e), title: `${e.name} is done`, body: e.detail ?? "Done", actions: ["Open"] } : null;
    case "error":
      return { ...base(e), title: `${e.name} stopped`, body: `Stopped: ${e.detail ?? "an error"}.${e.resetsAt ? ` Resets at ${e.resetsAt}.` : ""}`, actions: ["Open"] };
    case "needs-question":
      return { ...base(e), title: `${e.name} needs you`, body: "Has a question", actions: ["Open"] };
    case "needs-permission":
      // A risky request never shows its command in a banner: the user has to look at it in the window.
      if (e.risky) return { ...base(e), title: `${e.name} needs you`, body: "Needs a careful look.", actions: ["Review"] };
      // Allow and Deny buttons need the OS to offer actions; on Linux only the click that opens the session is certain.
      return { ...base(e), title: `${e.name} needs you`, body: `Wants to run \`${e.detail ?? "a command"}\``, actions: os === "linux" ? ["Open"] : ["Allow", "Deny"] };
  }
}

const list = (names: string[]) => (names.length <= 1 ? names.join("") : `${names.slice(0, -1).join(", ")} and ${names[names.length - 1]}`);

/** Several requests within BATCH_WINDOW_MS become one banner, and any risky one in it makes the only action Review. */
export function batchBanner(events: NotifyEvent[], os: Os): Banner | null {
  const live = events.filter((e) => bannerFor(e, os));
  if (live.length === 0) return null;
  if (live.length === 1) return bannerFor(live[0]!, os);
  const risky = live.some((e) => e.risky);
  return { ...base(live[0]!), meta: `${live.length} sessions`, title: `${live.length} sessions need you`, body: list(live.map((e) => e.name)), actions: risky ? ["Review"] : ["Open Marshell"] };
}

/** The one escalation, after a wait: how many minutes the session has been waiting. */
export function escalationBanner(e: NotifyEvent, os: Os): Banner {
  const min = Math.max(1, Math.round((e.waitedMs ?? 60_000) / 60_000));
  const what = e.kind === "needs-question" ? "for an answer" : "to run a command";
  const first = bannerFor({ ...e, windowFocused: false }, os) ?? { ...base(e), title: `${e.name} needs you`, actions: ["Open"], body: "" };
  return { ...first, body: `${e.name} has waited ${min} min ${what}` };
}

/** Banners are OS UI and sound: the window announces nothing extra for them. */
export function bannerAnnouncement(): null {
  return null;
}
