import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { focusOrder } from "../../lib/focusOrder";
import { FixSheet } from "./FixSheet";
import { DOCTOR_REPORT } from "./fixtures";

describe("FixSheet", () => {
  const h = renderToStaticMarkup(<FixSheet report={DOCTOR_REPORT} />);
  it("shows the combined diff grouped by file, with the backup and the way back", () => {
    expect(h).toContain("C:\\Users\\Home Office\\.claude\\settings.json");
    expect(h).toContain("diff__lines");
    expect(h).toContain("A backup is saved first");
    expect(h).toContain("Undo last fix");
  });
  it("promises the backup only for as long as the files are unchanged since this fix, and never a removal of 'only the entries added'", () => {
    expect(h).toContain("puts that backup back, for as long as the files are unchanged since this fix");
    expect(h).not.toMatch(/only the entries|byte for byte/);
  });
  it("its scrolling body is a focusable labelled region, first in the tab order before Cancel and the fix", () => {
    expect(h).toMatch(/<div class="fix-sheet__body" role="region" aria-label="Changes to review" tabindex="0">/);
    expect(focusOrder(h)).toEqual(["Changes to review", "Cancel", "Fix 4 issues"]);
  });
  it("has Cancel and the fix as its two buttons, and the fix names the count", () => {
    expect(h).toContain(">Cancel<");
    expect(h).toContain(">Fix 4 issues<");
  });
  it("is a dialog labelled by its title and takes focus on the title, not on an action", () => {
    expect(h).toContain('role="dialog"');
    expect(h).toContain('tabindex="-1"');
    expect(h).not.toMatch(/autofocus/i);
  });
  it("names the sessions that will need a restart", () => {
    expect(h).toContain("Sessions 3 and 5");
  });
});
