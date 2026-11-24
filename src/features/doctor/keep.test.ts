import { describe, expect, it } from "vitest";
import { ApiError } from "../../lib/api/client";
import { keepLabel, keepName, pressKeep } from "./keep";

describe("keep buttons", () => {
  it("label the action, and say what is happening while the request runs", () => {
    expect(keepLabel("keep", false)).toBe("Keep my file");
    expect(keepLabel("stopKeeping", false)).toBe("Stop keeping");
    expect(keepLabel("keep", true)).toBe("Keeping…");
    expect(keepLabel("stopKeeping", true)).toBe("Stopping…");
  });
  it("name the button with the row's title after the visible label", () => {
    expect(keepName("keep", "T", false)).toBe("Keep my file: T");
    expect(keepName("stopKeeping", "T", false)).toBe("Stop keeping: T");
    expect(keepName("keep", "T", true)).toBe("Keeping…: T");
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
});
