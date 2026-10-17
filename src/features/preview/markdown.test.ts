import { describe, expect, it } from "vitest";
import { renderMarkdown, slugOf } from "./markdown";

describe("renderMarkdown", () => {
  it("escapes raw HTML instead of passing it through", () => {
    const out = renderMarkdown("<script>alert(1)</script>\n\n<img src=x onerror=alert(1)>");
    expect(out).not.toContain("<script");
    expect(out).not.toContain("<img");
    expect(out).toContain("&lt;script&gt;");
  });
  it("gives a heading a GitHub-style id", () => {
    expect(renderMarkdown("## Set up")).toContain('<h2 id="set-up">Set up</h2>');
    expect(renderMarkdown("# What's `new` in v2.0?")).toContain('id="whats-new-in-v20"');
  });
  it("numbers a repeated heading, and a heading that already looks numbered", () => {
    const out = renderMarkdown("# Intro\n\n# Intro\n\n# Intro\n\n# Intro 1");
    expect(out).toContain('id="intro"');
    expect(out).toContain('id="intro-1"');
    expect(out).toContain('id="intro-2"');
    expect(out).toContain('id="intro-1-1"');
  });
  it("starts the ids fresh for each render", () => {
    renderMarkdown("# Same");
    expect(renderMarkdown("# Same")).toContain('id="same"');
  });
  it("keeps the language class on a fenced block", () => {
    expect(renderMarkdown("```rust\nfn main() {}\n```")).toContain('<code class="language-rust">');
  });
  it("links bare addresses", () => {
    expect(renderMarkdown("see https://example.com now")).toContain('<a href="https://example.com">');
  });
  it("does not turn a javascript: link into a link", () => {
    expect(renderMarkdown("[x](javascript:alert(1))")).not.toContain("<a ");
  });
});

describe("slugOf", () => {
  it("lowercases, drops punctuation, turns spaces to hyphens and keeps non-ASCII letters", () => {
    expect(slugOf("Hello, World!")).toBe("hello-world");
    expect(slugOf("Café  au lait")).toBe("café--au-lait");
    expect(slugOf("snake_case & kebab-case")).toBe("snake_case--kebab-case");
    expect(slugOf("日本語 の 見出し")).toBe("日本語-の-見出し");
  });
});
