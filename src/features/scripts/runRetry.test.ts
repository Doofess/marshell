import { describe, expect, it } from "vitest";
import { scriptsFixture } from "./fixtures";
import { afterReread, askOnRunFailure } from "./runRetry";

describe("askOnRunFailure", () => {
  it("asks again on a 409 or 403 the first time", () => {
    expect(askOnRunFailure(409, false)).toBe(true);
    expect(askOnRunFailure(403, false)).toBe(true);
  });
  it("announces instead on a retried call, and for any other status", () => {
    expect(askOnRunFailure(409, true)).toBe(false);
    expect(askOnRunFailure(403, true)).toBe(false);
    expect(askOnRunFailure(500, false)).toBe(false);
    expect(askOnRunFailure(undefined, false)).toBe(false);
  });
});

describe("afterReread", () => {
  it("asks for an untrusted file with a run script", () => {
    const s = scriptsFixture({ trusted: false });
    expect(afterReread(s)).toEqual({ kind: "ask", scripts: s });
  });
  it("starts a file that is trusted now", () => {
    const s = scriptsFixture({ trusted: true });
    expect(afterReread(s)).toEqual({ kind: "start", scripts: s });
  });
  it("closes when there is nothing to run", () => {
    expect(afterReread(null)).toEqual({ kind: "close" });
    expect(afterReread(scriptsFixture({ run: null }))).toEqual({ kind: "close" });
    expect(afterReread(scriptsFixture({ problem: "bad" }))).toEqual({ kind: "close" });
    expect(afterReread(scriptsFixture({ hash: null }))).toEqual({ kind: "close" });
  });
});
