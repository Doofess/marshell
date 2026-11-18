import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";

const read = (file: string) => readFileSync(new URL(`../../../${file}`, import.meta.url), "utf8");

describe("packages", () => {
  it("the_notices_name_every_codemirror_package", () => {
    const names = new Set(read("pnpm-lock.yaml").match(/@(?:codemirror|lezer)\/[a-z-]+(?=@\d)/g));
    expect(names.size).toBeGreaterThan(0);
    const notices = read("THIRD_PARTY_NOTICES.md");
    expect([...names].filter((name) => !notices.includes(name))).toEqual([]);
  });
  it("index_html_has_no_style_element", () => {
    // A <style> in index.html makes Tauri add a nonce to style-src, which turns CodeMirror's injected styles off.
    expect(read("index.html")).not.toMatch(/<style/i);
  });
});
