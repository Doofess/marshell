import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { nameContainsLabel } from "../../lib/labelInName";
import { bannerFor } from "./banner";
import { OsBanner } from "./OsBanner";

const perm = { kind: "needs-permission" as const, number: 3, name: "api-server", project: "my-app", detail: "npm test" };
const OSES = ["windows", "mac", "linux"] as const;
const html = (os: (typeof OSES)[number], risky = false) => renderToStaticMarkup(<OsBanner os={os} banner={bannerFor({ ...perm, risky }, os)!} />);

describe("OsBanner", () => {
  for (const os of OSES) {
    it(`${os}: says which OS frame it is, that it never raises the window, who it is from, and what it says`, () => {
      const h = html(os);
      expect(h).toContain(`data-os="${os}"`);
      expect(h).toContain('data-raises-window="false"');
      expect(h).toContain(">Marshell<");
      expect(h).toContain("api-server needs you");
      expect(h).toContain("Wants to run `npm test`");
      expect(h).toContain("Session 3 · my-app");
    });
    it(`${os}: one button per action, each named by its own text`, () => {
      const h = html(os);
      const buttons = h.match(/<button[\s\S]*?<\/button>/g) ?? [];
      expect(buttons).toHaveLength(os === "linux" ? 1 : 2);
      for (const b of buttons) expect(nameContainsLabel(b)).toBe(true);
    });
  }
  it("a risky banner has exactly one button, Review, and never the command", () => {
    for (const os of OSES) {
      const h = html(os, true);
      expect(h.match(/<button/g)).toHaveLength(1);
      expect(h).toMatch(/<button[^>]*>Review<\/button>/);
      expect(h).not.toContain("npm test");
    }
  });
  it("lets names read in their own direction and carries no live region of its own", () => {
    expect(html("windows")).toContain('dir="auto"');
    expect(html("windows")).not.toMatch(/aria-live|role="status"|role="alert"/);
  });
  it("is a neutral layout, not the system's own styling: no OS logo, and it says so", () => {
    for (const os of OSES) expect(renderToStaticMarkup(<OsBanner os={os} banner={bannerFor(perm, os)!} caption />)).toContain("Layout and content, not the operating system");
  });
  it("marks macOS and Linux frames as design only, outside the banner itself", () => {
    expect(renderToStaticMarkup(<OsBanner os="mac" banner={bannerFor(perm, "mac")!} caption />)).toContain("Design only: macOS and Linux are not tested yet.");
    expect(renderToStaticMarkup(<OsBanner os="windows" banner={bannerFor(perm, "windows")!} caption />)).not.toContain("Design only");
  });
});
