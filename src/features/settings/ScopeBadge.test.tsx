import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { SCOPES } from "./scope";
import { ScopeBadge } from "./ScopeBadge";

describe("ScopeBadge", () => {
  it("says the scope in a word and draws its shape, and explains it on hover", () => {
    for (const s of SCOPES) {
      const h = renderToStaticMarkup(<ScopeBadge scope={s} />);
      expect(h).toContain(`data-scope="${s}"`);
      expect(h).toMatch(/<svg/);
      expect(h).toContain("title=");
    }
    expect(renderToStaticMarkup(<ScopeBadge scope="managed" />)).toContain("Managed");
  });
  it("reads as one phrase to a screen reader: the word, then what it means", () => {
    const h = renderToStaticMarkup(<ScopeBadge scope="managed" />);
    expect(h).toContain("Managed");
    expect(h).toContain("Set by your organisation.");
  });
});
