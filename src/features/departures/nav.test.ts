import { describe, expect, it } from "vitest";
import { boardRows } from "./board";
import { SCENARIO_ROWS } from "../window/scenario";
import { keyHint, reduceBoard } from "./nav";

const rows = boardRows(SCENARIO_ROWS);
const gateIdx = rows.findIndex((r) => r.gate);
const plainIdx = rows.findIndex((r) => !r.gate);
describe("reduceBoard", () => {
  it("moves with the arrows and stops at the ends", () => {
    expect(reduceBoard({ active: 0, card: null }, "ArrowDown", rows).state.active).toBe(1);
    expect(reduceBoard({ active: 0, card: null }, "ArrowUp", rows).state.active).toBe(0);
    expect(reduceBoard({ active: 0, card: null }, "End", rows).state.active).toBe(rows.length - 1);
  });
  it("Enter jumps to the session and never answers a request", () => {
    const r = reduceBoard({ active: gateIdx, card: null }, "Enter", rows);
    expect(r.jump).toBe(rows[gateIdx]!.id);
    expect(r.state.card).toBeNull();
  });
  it("Y and N open the inline card only on a row that needs you", () => {
    expect(reduceBoard({ active: gateIdx, card: null }, "KeyY", rows).state.card).toBe(rows[gateIdx]!.id);
    expect(reduceBoard({ active: gateIdx, card: null }, "KeyN", rows).state.card).toBe(rows[gateIdx]!.id);
    expect(reduceBoard({ active: plainIdx, card: null }, "KeyY", rows).state.card).toBeNull();
  });
  it("Escape closes an open card and otherwise does nothing", () => {
    expect(reduceBoard({ active: 0, card: "x" }, "Escape", rows).state.card).toBeNull();
    expect(reduceBoard({ active: 2, card: null }, "Escape", rows).state).toEqual({ active: 2, card: null });
  });
  it("copes with no rows", () => {
    expect(reduceBoard({ active: 0, card: null }, "ArrowDown", []).state.active).toBe(0);
  });
});

describe("reduceBoard: where focus lands in the opened request", () => {
  it("Y opens the request with focus on the card itself, never on Allow", () => {
    expect(reduceBoard({ active: gateIdx, card: null }, "KeyY", rows).state.focus).toBe("title");
  });
  it("N opens the same request with focus on Deny, and never denies by itself", () => {
    const r = reduceBoard({ active: gateIdx, card: null }, "KeyN", rows);
    expect(r.state.card).toBe(rows[gateIdx]!.id);
    expect(r.state.focus).toBe("deny");
  });
  it("closing the request forgets where focus was", () => {
    expect(reduceBoard({ active: 1, card: "x", focus: "deny" }, "Escape", rows).state).toEqual({ active: 1, card: null });
  });
});

describe("keyHint: the letter on the user's own key", () => {
  it("is the letter the layout prints on the physical key, when the layout says so", () => {
    const qwertz = new Map([["KeyY", "z"], ["KeyN", "n"]]);
    expect(keyHint("KeyY", qwertz)).toBe("Z");
    expect(keyHint("KeyN", qwertz)).toBe("N");
  });
  it("falls back to the code's own letter when the layout is not known", () => {
    expect(keyHint("KeyY")).toBe("Y");
    expect(keyHint("KeyN", new Map())).toBe("N");
  });
});
