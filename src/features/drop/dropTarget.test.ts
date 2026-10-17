import { describe, expect, it } from "vitest";
import type { RowModel } from "../sidebar/types";
import { deliveryTo, dropDelivery, dropTarget, toCss } from "./dropTarget";

const row = (status: string, agent = "claude"): RowModel => ({ name: "api", status, agent }) as unknown as RowModel;

describe("dropTarget", () => {
  it("is the terminal whose host the element is inside", () => {
    const el = { closest: (sel: string) => (sel === "[data-drop-tab]" ? { getAttribute: () => "t3" } : null) };
    expect(dropTarget(el)).toEqual({ kind: "terminal", tabId: "t3" });
  });
  it("is the preview anywhere else, and with no element", () => {
    expect(dropTarget({ closest: () => null })).toEqual({ kind: "preview" });
    expect(dropTarget(null)).toEqual({ kind: "preview" });
  });
});

describe("toCss", () => {
  it("divides physical pixels by the device pixel ratio", () => {
    expect(toCss({ x: 300, y: 150 }, 1.5)).toEqual({ x: 200, y: 100 });
  });
});

describe("dropDelivery", () => {
  it("refuses a paused and an ended session with the 2.5b copy", () => {
    expect(dropDelivery(row("paused"))).toEqual({ name: "api", refused: "api is paused." });
    expect(dropDelivery(row("ended"))).toEqual({ name: "api", refused: "api has ended." });
  });
  it("accepts any other live session, agent or shell", () => {
    expect(dropDelivery(row("working"))).toEqual({ name: "api" });
    expect(dropDelivery(row("idle", "shell"))).toEqual({ name: "api" });
  });
  it("is null with no row", () => {
    expect(dropDelivery(null)).toBeNull();
  });
});

describe("deliveryTo", () => {
  const rows = [{ ...row("paused"), id: "t1" }] as unknown as RowModel[];
  it("is the shell panel for its tab, which has no sidebar row", () => {
    expect(deliveryTo("p9", rows, "p9")).toEqual({ name: "the shell panel", panel: true });
  });
  it("is the row's delivery for any other tab", () => {
    expect(deliveryTo("t1", rows, "p9")).toEqual({ name: "api", refused: "api is paused." });
    expect(deliveryTo("t2", rows, undefined)).toBeNull();
  });
});
