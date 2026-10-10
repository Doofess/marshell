import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
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
