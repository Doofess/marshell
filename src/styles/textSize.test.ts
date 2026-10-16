import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";
import { parseTokens } from "../design/cssTokens";

// Read from disk: Vitest hands CSS imports to tests as empty strings.
const css = readFileSync(new URL("./appearance.css", import.meta.url), "utf8");

const SIZES = {
  large: { "--text-11": "0.75rem", "--text-12": "0.8125rem", "--text-13": "0.9375rem", "--text-15": "1.0625rem", "--text-20": "1.375rem", "--text-28": "1.875rem" },
  larger: { "--text-11": "0.8125rem", "--text-12": "0.9375rem", "--text-13": "1.0625rem", "--text-15": "1.25rem", "--text-20": "1.5rem", "--text-28": "2rem" },
};

describe("text size", () => {
  for (const [size, table] of Object.entries(SIZES))
    it(`${size} moves the type scale up`, () => {
      const t = parseTokens(css, `:root[data-text-size="${size}"]`);
      for (const [token, value] of Object.entries(table)) expect(t[token]?.light, `${size} ${token}`).toBe(value);
    });
});
