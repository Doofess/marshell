import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { FIXTURES } from "../features/sidebar/fixtures";
import { SessionRow } from "../features/sidebar/SessionRow";
import type { Density } from "../features/sidebar/types";
import { AllSessionsMenu } from "../features/window/AllSessionsMenu";
import { Palette } from "../features/palette/Palette";
import { PALETTE_SESSIONS } from "../features/palette/fixtures";
import { nameContainsLabel, visibleText } from "./labelInName";

describe("the check itself (as axe makes it)", () => {
  it("reads neighbouring text nodes as one word, ignoring case and punctuation", () => {
    expect(visibleText('<span aria-hidden="true">10</span><span>Docs-Site</span> marketing')).toBe("10docssite marketing");
  });
  it("fails a name that puts a comma-and-space where the screen has none", () => {
    expect(nameContainsLabel('<div aria-label="Session 10, docs-site">' + "<span>10</span><span>docs-site</span></div>")).toBe(false);
  });
  it("passes when whitespace separates the pieces on screen", () => {
    expect(nameContainsLabel('<div aria-label="Session 10, docs-site">' + "<span>10</span> <span>docs-site</span></div>")).toBe(true);
  });
});

describe("label in name: every control's name contains the text it shows", () => {
  for (const density of ["compact", "comfortable", "expanded"] as Density[]) {
    it(`sidebar rows, ${density}`, () => {
      const bad = FIXTURES.filter((row) => !nameContainsLabel(renderToStaticMarkup(<SessionRow row={row} density={density} />))).map((r) => r.id);
      expect(bad).toEqual([]);
    });
  }

  it("all-sessions menu rows", () => {
    const html = renderToStaticMarkup(<AllSessionsMenu rows={FIXTURES} />);
    const rows = html.split('<button type="button" class="all-sessions__row is-selected-mark"').slice(1).map((s) => '<button type="button" class="all-sessions__row is-selected-mark"' + s.split("</button>")[0] + "</button>");
    expect(rows.length).toBe(FIXTURES.length);
    expect(rows.filter((r) => !nameContainsLabel(r))).toEqual([]);
  });

  it("palette session rows, with their Ctrl+digit", () => {
    const html = renderToStaticMarkup(<Palette view={{ query: "", activeIndex: 0 }} sessions={PALETTE_SESSIONS} />);
    const rows = html.split(/<li id="[^"]*-opt-/).slice(1).map((s) => "<li " + s.split("</li>")[0] + "</li>").filter((r) => r.includes('data-kind="session"'));
    expect(rows.length).toBeGreaterThan(0);
    expect(rows.filter((r) => !nameContainsLabel(r))).toEqual([]);
  });
});
