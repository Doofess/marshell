import { describe, expect, it } from "vitest";
import { parseTokens } from "./cssTokens";

const css = `
:root {
  --a: light-dark(oklch(50% 0.1 20), oklch(80% 0.1 20));
  --b: 13px;
  --c: color-mix(in oklch, var(--a) 10%, transparent);
}
:root,
[data-agent="codex"] {
  --accent: light-dark(oklch(58% 0.15 65), oklch(80% 0.16 72));
}
[data-agent="claude"] { --accent: light-dark(oklch(54% 0.19 258), oklch(70% 0.15 250)); }
`;

describe("parseTokens", () => {
  it("splits light-dark into both themes", () => {
    expect(parseTokens(css, ":root")["--a"]).toEqual({ light: "oklch(50% 0.1 20)", dark: "oklch(80% 0.1 20)" });
  });
  it("keeps plain values for both themes", () => {
    expect(parseTokens(css, ":root")["--b"]).toEqual({ light: "13px", dark: "13px" });
  });
  it("skips values that depend on var()", () => {
    expect(parseTokens(css, ":root")["--c"]).toBeUndefined();
  });
  it("finds a selector inside a selector list", () => {
    expect(parseTokens(css, '[data-agent="codex"]')["--accent"]?.dark).toBe("oklch(80% 0.16 72)");
  });
  it("finds a single-line rule", () => {
    expect(parseTokens(css, '[data-agent="claude"]')["--accent"]?.light).toBe("oklch(54% 0.19 258)");
  });
  it("returns an empty object for a missing selector", () => {
    expect(parseTokens(css, '[data-agent="nope"]')).toEqual({});
  });
});
