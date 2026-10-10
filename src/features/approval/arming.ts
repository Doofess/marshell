export const ARM_DELAY_MS = 500;
export const HOLD_MS = 600;

export type CardAction = "allow" | "hold-allow" | "deny" | "always" | "terminal" | "expand" | "jump";
export type KeyContext = {
  /** Classified risky by the core. */
  risky: boolean;
  /** The 500 ms delay since focus or content change has passed. */
  armed: boolean;
  /** The core offered an exact rule. */
  hasAlways: boolean;
  /** The payload is longer than four lines. */
  truncated: boolean;
  expanded: boolean;
};

/** Buttons and keys are inert until 500 ms after the card gained focus or its content changed. */
export function isArmed(armedAtMs: number, nowMs: number): boolean {
  return nowMs - armedAtMs >= ARM_DELAY_MS;
}

export function holdProgress(heldMs: number): number {
  if (!Number.isFinite(heldMs) || heldMs <= 0) return 0;
  return Math.min(1, heldMs / HOLD_MS);
}

export function holdComplete(heldMs: number): boolean {
  return holdProgress(heldMs) >= 1;
}

/**
 * What a key does while the card has focus (docs/PLAN.md "Keyboard safety"). Enter never allows: it jumps to the
 * session. Unknown keys do nothing, so no terminal keystroke can ever act on a card.
 */
export function keyAction(key: string, ctx: KeyContext): CardAction | null {
  if (key === "Enter") return "jump";
  if (key === " ") return ctx.truncated && !ctx.expanded ? "expand" : null;
  if (!ctx.armed) return null;
  // Lines below the fold are lines the user has not seen: Allow and Always first expand the payload, on any request.
  const hidden = ctx.truncated && !ctx.expanded;
  switch (key.toLowerCase()) {
    case "y":
      if (hidden) return "expand";
      return ctx.risky ? "hold-allow" : "allow";
    case "n":
      return "deny";
    case "a":
      if (ctx.risky || !ctx.hasAlways) return null;
      return hidden ? "expand" : "always";
    case "t":
      return "terminal";
    default:
      return null;
  }
}
