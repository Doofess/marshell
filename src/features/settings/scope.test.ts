import { describe, expect, it } from "vitest";
import { isReadOnlyScope, SCOPE_LABEL, SCOPE_NOTE, SCOPE_SHAPE, SCOPES } from "./scope";

describe("scopes", () => {
  it("has the five scopes, each with a word, a sentence and a shape", () => {
    expect(SCOPES).toEqual(["user", "project", "local", "managed", "marshell"]);
    expect(SCOPES.map((s) => SCOPE_LABEL[s])).toEqual(["User", "Project", "Local", "Managed", "Marshell"]);
    expect(new Set(SCOPES.map((s) => SCOPE_SHAPE[s])).size).toBe(5);
    expect(SCOPE_NOTE.managed).toBe("Set by your organisation. Marshell can't change it.");
  });
  it("only managed is read-only", () => {
    expect(SCOPES.filter(isReadOnlyScope)).toEqual(["managed"]);
  });
});
