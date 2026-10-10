import { describe, expect, it } from "vitest";
import { ARM_DELAY_MS, HOLD_MS, holdComplete, holdProgress, isArmed, keyAction, type KeyContext } from "./arming";

const safe: KeyContext = { risky: false, armed: true, hasAlways: true, truncated: false, expanded: false };
const risky: KeyContext = { ...safe, risky: true, hasAlways: false };

describe("arming", () => {
  it("is inert for 500 ms after the card gains focus or its content changes", () => {
    expect(ARM_DELAY_MS).toBe(500);
    expect(isArmed(1000, 1499)).toBe(false);
    expect(isArmed(1000, 1500)).toBe(true);
  });
});

describe("hold to allow", () => {
  it("needs 600 ms", () => {
    expect(HOLD_MS).toBe(600);
    expect(holdProgress(0)).toBe(0);
    expect(holdProgress(300)).toBe(0.5);
    expect(holdProgress(5000)).toBe(1);
    expect(holdComplete(599)).toBe(false);
    expect(holdComplete(600)).toBe(true);
  });
  it("treats nonsense as no progress", () => {
    expect(holdProgress(-5)).toBe(0);
    expect(holdProgress(Number.NaN)).toBe(0);
  });
});

describe("keyAction on a safe request", () => {
  it("maps Y N A T", () => {
    expect(keyAction("y", safe)).toBe("allow");
    expect(keyAction("Y", safe)).toBe("allow");
    expect(keyAction("n", safe)).toBe("deny");
    expect(keyAction("a", safe)).toBe("always");
    expect(keyAction("t", safe)).toBe("terminal");
  });
  it("never offers Always when there is no rule", () => {
    expect(keyAction("a", { ...safe, hasAlways: false })).toBeNull();
  });
  it("is inert before the arming delay ends", () => {
    for (const k of ["y", "n", "a", "t"]) expect(keyAction(k, { ...safe, armed: false }), k).toBeNull();
  });
});

describe("Enter never allows (review focus 2)", () => {
  it("jumps to the session, even before arming and on a risky card", () => {
    expect(keyAction("Enter", safe)).toBe("jump");
    expect(keyAction("Enter", risky)).toBe("jump");
    expect(keyAction("Enter", { ...risky, armed: false })).toBe("jump");
  });
});

describe("risky requests", () => {
  it("turn Y into a hold, never an instant allow", () => {
    expect(keyAction("y", risky)).toBe("hold-allow");
  });
  it("hide Always", () => {
    expect(keyAction("a", { ...risky, hasAlways: true })).toBeNull();
  });
  it("must have a truncated payload expanded before Allow arms", () => {
    const long = { ...risky, truncated: true, expanded: false };
    expect(keyAction("y", long)).toBe("expand");
    expect(keyAction("y", { ...long, expanded: true })).toBe("hold-allow");
  });
  it("still let Deny through", () => {
    expect(keyAction("n", risky)).toBe("deny");
  });
});

describe("Space", () => {
  it("expands a truncated payload and does nothing otherwise", () => {
    expect(keyAction(" ", { ...safe, truncated: true })).toBe("expand");
    expect(keyAction(" ", { ...safe, truncated: true, expanded: true })).toBeNull();
    expect(keyAction(" ", safe)).toBeNull();
  });
});

describe("other keys", () => {
  it("do nothing, so terminal typing can never reach the card", () => {
    for (const k of ["x", "Escape", "Tab", "1", "Backspace"]) expect(keyAction(k, safe), k).toBeNull();
  });
});
