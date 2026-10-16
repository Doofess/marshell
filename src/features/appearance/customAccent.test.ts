import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";
import { contrast } from "../../design/contrast";
import { parseTokens } from "../../design/cssTokens";
import { ACCENT_SURFACES, checkAccent, normalizeHex } from "./customAccent";

it("uses the same surfaces as tokens.css", () => {
  const tokens = parseTokens(readFileSync(new URL("../../styles/tokens.css", import.meta.url), "utf8"), ":root");
  const surfaces = ["--bg-base", "--bg-raised", "--bg-overlay"];
  expect(ACCENT_SURFACES.light).toEqual(surfaces.map((s) => tokens[s]!.light));
  expect(ACCENT_SURFACES.dark).toEqual(surfaces.map((s) => tokens[s]!.dark));
});

describe("normalizeHex", () => {
  it("expands short hex and lower-cases", () => expect(normalizeHex("#ABC")).toBe("#aabbcc"));
  it("adds a missing #", () => expect(normalizeHex("3A7BD5")).toBe("#3a7bd5"));
  it("trims spaces", () => expect(normalizeHex("  #3a7bd5 ")).toBe("#3a7bd5"));
  it("rejects anything else", () => {
    expect(normalizeHex("#12345")).toBeNull();
    expect(normalizeHex("blue")).toBeNull();
    expect(normalizeHex("")).toBeNull();
  });
});

describe("checkAccent", () => {
  it("accepts a mid blue and picks readable text for it", () => {
    const r = checkAccent("#3A7BD5");
    expect(r.ok).toBe(true);
    if (!r.ok) return;
    expect(r.hex).toBe("#3a7bd5");
    expect(contrast(r.hex, r.ink)).toBeGreaterThanOrEqual(4.5);
  });
  it("rejects a bad format with an example", () => {
    expect(checkAccent("blue")).toEqual({ ok: false, reason: "Enter a colour as #rrggbb, for example #3a7bd5." });
  });
  it("rejects a colour too light for the light theme", () => {
    const r = checkAccent("#ffff66");
    expect(r.ok).toBe(false);
    if (r.ok) return;
    expect(r.reason).toMatch(/^Too light for the light theme: \d+\.\d:1 against the background, needs 3:1\.$/);
  });
  it("rejects a colour too dark for the dark theme", () => {
    const r = checkAccent("#202020");
    expect(r.ok).toBe(false);
    if (r.ok) return;
    expect(r.reason).toMatch(/^Too dark for the dark theme: \d+\.\d:1 against the background, needs 3:1\.$/);
  });
});
