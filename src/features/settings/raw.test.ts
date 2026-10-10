import { describe, expect, it } from "vitest";
import { parseRaw } from "./raw";

describe("parseRaw", () => {
  it("returns the object for valid JSON", () => expect(parseRaw('{"a":1}')).toEqual({ ok: true, value: { a: 1 } }));
  it("names the line and column of a mistake", () => {
    const r = parseRaw('{\n  "a": 1,\n  "b": }\n');
    expect(r.ok).toBe(false);
    if (!r.ok) {
      expect(r.line).toBe(3);
      expect(r.column).toBeGreaterThan(0);
      expect(r.message.length).toBeGreaterThan(0);
    }
  });
  it("refuses a value that is not an object, and an empty file", () => {
    expect(parseRaw("[1]").ok).toBe(false);
    expect(parseRaw("").ok).toBe(false);
  });
});
