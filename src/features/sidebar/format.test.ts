import { describe, expect, it } from "vitest";
import { formatCost, formatDuration, formatTokens, formatUsage, modeCaution } from "./format";

describe("formatDuration", () => {
  it.each([
    [undefined, "–"],
    [Number.NaN, "–"],
    [-1, "–"],
    [0, "0s"],
    [59_400, "59s"],
    [60_000, "1m"],
    [59 * 60_000 + 59_000, "59m"],
    [3_600_000, "1h 00m"],
    [3_900_000, "1h 05m"],
    [26 * 3_600_000 + 20 * 60_000, "1d 2h"],
  ])("%s ms → %s", (ms, out) => expect(formatDuration(ms)).toBe(out));
});

describe("formatTokens", () => {
  it.each([
    [undefined, "–"],
    [0, "0"],
    [312, "312"],
    [999, "999"],
    [1_000, "1k"],
    [84_400, "84k"],
    [999_499, "999k"],
    [1_200_000, "1.2M"],
    [12_000_000, "12M"],
  ])("%s → %s", (n, out) => expect(formatTokens(n)).toBe(out));
});

describe("formatCost", () => {
  it.each([
    [undefined, "–"],
    [0.04, "$0.04"],
    [1.2, "$1.20"],
    [1234.56, "$1,234.56"],
  ])("%s → %s", (n, out) => expect(formatCost(n)).toBe(out));
});

describe("formatUsage", () => {
  it("joins all parts", () => {
    expect(formatUsage({ inContext: 84_000, window: 200_000, tokensIn: 312_000, tokensOut: 48_000, costUsd: 1.2 })).toBe(
      "84k / 200k · 312k in · 48k out · $1.20",
    );
  });
  it("marks unknown parts with a dash", () => {
    expect(formatUsage({ inContext: 84_000, tokensIn: 312_000 })).toBe("84k / – · 312k in · – out · –");
  });
  it("is a single dash when nothing is known", () => {
    expect(formatUsage(undefined)).toBe("–");
  });
});

describe("modeCaution", () => {
  it("names only the modes that run without asking", () => {
    expect(["manual", "plan", "auto-edit", "full-auto", "bypass", undefined].map((m) => modeCaution(m as never))).toEqual([
      null,
      null,
      "Auto-accept edits",
      "Full auto",
      "Bypass permissions",
      null,
    ]);
  });
});
