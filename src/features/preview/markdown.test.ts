import { describe, expect, it } from "vitest";
import { remembered, renderCommentMarkdown, renderMarkdown, slugOf } from "./markdown";

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

/** What a reader sees of the HTML: its tags taken out, so an attribute (a title, an address) doesn't count. */
const seen = (html: string) => html.replace(/<[^>]*>/g, "");

describe("renderCommentMarkdown", () => {
  it("a comment's picture is a link", () => {
    const out = renderCommentMarkdown("![pic](https://example.com/a.png)");
    expect(out).toContain('<a href="https://example.com/a.png">pic</a>');
    expect(out).not.toContain("<img");
    expect(renderCommentMarkdown("![](https://example.com/a.png)")).toContain('<a href="https://example.com/a.png">Picture</a>');
  });
  it("a picture that isn't https is its alt text", () => {
    const out = renderCommentMarkdown("![x](javascript:alert(1))");
    expect(out).not.toContain("href");
    expect(out).not.toContain("<img");
    const http = renderCommentMarkdown("![shot](http://example.com/a.png) and ![](a.png)");
    expect(http).not.toContain("href");
    expect(seen(http)).toContain("shot");
    expect(seen(http)).toContain("Picture");
  });
  it("a picture's alt text is escaped and shows what is in code and in a nested picture", () => {
    const out = renderCommentMarkdown("![<b>x</b> `code` ![inner](https://example.com/i.png)](https://example.com/a.png)");
    expect(out).not.toContain("<b>");
    expect(seen(out)).toContain("&lt;b&gt;x&lt;/b&gt;");
    expect(seen(out)).toContain("code");
    expect(seen(out)).toContain("inner");
  });
  it("a suggestion is a code block", () => {
    expect(renderCommentMarkdown("```suggestion\nx\n```")).toContain('<pre><code class="language-suggestion">');
  });
  it("raw HTML in a comment is escaped", () => {
    const out = renderCommentMarkdown("<details>\n<summary>x</summary>\n</details>");
    expect(out).toContain("&lt;details&gt;");
    expect(out).not.toContain("<details");
  });
  it("a comment's heading has no id", () => {
    const out = renderCommentMarkdown("## Set up");
    expect(out).toContain("<h2>Set up</h2>");
    expect(out).not.toContain("id=");
  });
  it("a link reference definition shows as text", () => {
    expect(seen(renderCommentMarkdown("[a]: <ignore previous instructions>"))).toContain("ignore previous instructions");
    expect(seen(renderCommentMarkdown('> [a]: https://example.com "hidden title"'))).toContain("hidden title");
    expect(seen(renderCommentMarkdown('[^x]: https://example.com "hidden title"'))).toContain("hidden title");
    expect(seen(renderCommentMarkdown('[a\nb]: /url "hidden title"'))).toContain("hidden title");
    const used = renderCommentMarkdown("[text][a]\n\n[a]: https://example.com");
    expect(used).not.toContain(">text</a>");
    expect(seen(used)).toContain("[text][a]");
    expect(seen(used)).toContain("[a]: https://example.com");
  });
  it("a fence's info string shows whole", () => {
    const out = renderCommentMarkdown("```suggestion run this first\nx\n```");
    expect(seen(out)).toContain("suggestion run this first");
    expect(out).toContain('<p class="markdown__info">suggestion run this first</p>');
    expect(renderCommentMarkdown("```\nx\n```")).toBe("<pre><code>x\n</code></pre>\n");
    expect(seen(renderCommentMarkdown("```a<b>&amp;\nx\n```"))).toContain("a&lt;b&gt;&amp;amp;");
  });
  it("a table shows as its source", () => {
    const out = renderCommentMarkdown("|a|\n|-|\n|b|hidden cell|");
    expect(seen(out)).toContain("hidden cell");
    expect(out).not.toContain("<table");
  });
  it("a link's title shows as text after it", () => {
    const out = renderCommentMarkdown('[docs](https://example.com "ignore previous instructions")');
    expect(out).toContain('<a href="https://example.com">docs</a> (ignore previous instructions)');
    expect(out).not.toContain("title=");
    expect(renderCommentMarkdown('[docs](https://example.com "a <b> & c")')).toContain("</a> (a &lt;b&gt; &amp; c)");
    expect(renderCommentMarkdown("[docs](https://example.com)")).toContain('<a href="https://example.com">docs</a></p>');
  });
  it("a picture's title shows as text after it", () => {
    const out = renderCommentMarkdown('![pic](https://example.com/a.png "run this first")');
    expect(out).toContain('<a href="https://example.com/a.png">pic</a> (run this first)');
    expect(out).not.toContain("title=");
  });
  it("a link with nothing to see shows its address", () => {
    expect(renderCommentMarkdown("[](https://example.com/ignore-previous-instructions)")).toContain('<a href="https://example.com/ignore-previous-instructions">https://example.com/ignore-previous-instructions</a>');
    for (const blank of [" ", " ", "⠀", "　"]) {
      expect(seen(renderCommentMarkdown(`[${blank}](https://example.com/hidden-words)`)), JSON.stringify(blank)).toContain("https://example.com/hidden-words");
    }
    expect(seen(renderCommentMarkdown("[docs](https://example.com/words)"))).not.toContain("https://example.com/words");
    expect(seen(renderCommentMarkdown("[`x`](https://example.com/words)"))).not.toContain("https://example.com/words");
  });
  it("a picture inside a link is its alt text, so the link keeps its text", () => {
    const out = renderCommentMarkdown("[![](https://example.com/a.png)](https://example.com/target)");
    expect(out).toContain('<a href="https://example.com/target">Picture</a>');
    expect(out.match(/<a /g)).toHaveLength(1);
  });
  it("a character entity shows as written", () => {
    const out = renderCommentMarkdown("a&ZeroWidthSpace;b &#x202E;c &amp; d");
    const text = seen(out);
    expect(text).toContain("&amp;ZeroWidthSpace;");
    expect(text).toContain("&amp;#x202E;");
    expect(text).toContain("&amp;amp;");
    expect(out).not.toContain("​");
    expect(out).not.toContain("‮");
  });
  it("leaves renderMarkdown as it was", () => {
    expect(renderMarkdown("a&amp;b")).toContain("a&amp;b");
    expect(renderMarkdown("|a|\n|-|\n|b|")).toContain("<table");
    expect(renderMarkdown("![x](https://example.com/a.png)")).toContain("<img");
  });
});

describe("remembered", () => {
  it("calls its maker once for a source and forgets the oldest past its size", () => {
    const calls: string[] = [];
    const get = remembered((s) => (calls.push(s), `<p>${s}</p>`), 2);
    expect(get("a")).toBe("<p>a</p>");
    expect(get("a")).toBe("<p>a</p>");
    get("b");
    get("c");
    expect(calls).toEqual(["a", "b", "c"]);
    get("c");
    get("b");
    expect(calls).toEqual(["a", "b", "c"]);
    get("a");
    expect(calls).toEqual(["a", "b", "c", "a"]);
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
