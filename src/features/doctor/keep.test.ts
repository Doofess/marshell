import { describe, expect, it } from "vitest";
import { ApiError } from "../../lib/api/client";
import { keepDone, keepFocusKey, keepFocusLost, keepLabel, keepName, pressKeep } from "./keep";

describe("keep buttons", () => {
  it("label the action, and say what is happening while the request runs", () => {
    expect(keepLabel("keep", false)).toBe("Keep my file");
    expect(keepLabel("stopKeeping", false)).toBe("Stop keeping");
    expect(keepLabel("keep", true)).toBe("Keeping…");
    expect(keepLabel("stopKeeping", true)).toBe("Stopping…");
  });
  it("label Use this copy's text, and say Preparing while the plan is made", () => {
    expect(keepLabel("useCopy", false)).toBe("Use this copy's text");
    expect(keepLabel("useCopy", true)).toBe("Preparing…");
    expect(keepName("useCopy", "T", false)).toBe("Use this copy's text: T");
  });
  it("name the button with the row's title after the visible label", () => {
    expect(keepName("keep", "T", false)).toBe("Keep my file: T");
    expect(keepName("stopKeeping", "T", false)).toBe("Stop keeping: T");
    expect(keepName("keep", "T", true)).toBe("Keeping…: T");
  });
  it("the focus counts as lost on nothing, the body or the page itself, and not on another control", () => {
    // The check a press starts takes the rows away for a moment, and the focus falls back to the page (live check).
    const [body, page, other] = [{}, {}, {}];
    expect(keepFocusLost(null, body, page)).toBe(true);
    expect(keepFocusLost(body, body, page)).toBe(true);
    expect(keepFocusLost(page, body, page)).toBe(true);
    expect(keepFocusLost(other, body, page)).toBe(false);
    expect(keepFocusLost(other, body, null)).toBe(false);
  });
  it("pressKeep loads the report when the run it asked for is already over", async () => {
    const calls: string[] = [];
    const r = await pressKeep(async () => ({ running: false }) as never, async () => void calls.push("load"));
    expect(r).toEqual({ ok: true });
    expect(calls).toEqual(["load"]);
  });
  it("pressKeep leaves the report to the run's frame while it runs", async () => {
    const calls: string[] = [];
    expect(await pressKeep(async () => ({ running: true }) as never, async () => void calls.push("load"))).toEqual({ ok: true });
    expect(calls).toEqual([]);
  });
  it("pressKeep returns the server's message on a refusal, and a generic one without", async () => {
    const load = async () => {};
    expect(await pressKeep(() => Promise.reject(new ApiError(422, "This item can't be fixed any more. Run the check again.")), load)).toEqual({ ok: false, message: "This item can't be fixed any more. Run the check again." });
    expect(await pressKeep(() => Promise.reject(new ApiError(500, "")), load)).toEqual({ ok: false, message: "Marshell couldn't change that." });
  });
  it("keepFocusKey is the row's CLI and distro, so the row that replaces a pressed one is found", () => {
    expect(keepFocusKey({ agent: "copilot" })).toBe("copilot|");
    expect(keepFocusKey({ agent: "copilot", distro: "Ubuntu" })).toBe("copilot|Ubuntu");
    expect(keepFocusKey({})).toBe("|");
  });
  it("keepDone says what Marshell now does", () => {
    expect(keepDone("keep")).toBe("Marshell keeps your file as it is.");
    expect(keepDone("stopKeeping")).toBe("Marshell no longer keeps that file.");
  });
});
