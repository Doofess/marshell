import { describe, expect, it } from "vitest";
import { parseJsonc } from "./jsonc";

describe("parseJsonc", () => {
  it("strips comments and trailing commas", () => {
    const text = `{
      // a line comment
      "a": 1, /* a block
      comment */
      "b": [1, 2,],
      "c": { "d": 3, },
    }`;
    expect(parseJsonc(text)).toEqual({ a: 1, b: [1, 2], c: { d: 3 } });
  });

  it("strips BOM", () => expect(parseJsonc('﻿{"a":1}')).toEqual({ a: 1 }));

  it("jsonc_keeps_slashes_inside_strings", () => {
    expect(parseJsonc('{"u":"https://x//y"}')).toEqual({ u: "https://x//y" });
    expect(parseJsonc('{"u":"a /* b */ c, ]","v":"q\\"//r"}')).toEqual({ u: "a /* b */ c, ]", v: 'q"//r' });
  });

  it("error says Not valid JSON", () => {
    expect(() => parseJsonc("{nope")).toThrow(/^Not valid JSON: /);
  });
});
