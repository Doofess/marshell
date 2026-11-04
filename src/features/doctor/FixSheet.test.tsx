import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { focusOrder } from "../../lib/focusOrder";
import { DifferencesSheet, FixSheet } from "./FixSheet";
import { DOCTOR_PLAN, DOCTOR_PREVIEW, filePreview } from "./fixtures";

const noop = () => {};
const sheet = (over: Partial<Parameters<typeof FixSheet>[0]> = {}) => renderToStaticMarkup(<FixSheet plan={DOCTOR_PLAN} busy={false} conflict={false} onConfirm={noop} onCancel={noop} onReload={noop} {...over} />);

describe("FixSheet", () => {
  const h = sheet();
  it("shows one labelled diff per changed file, with the backup and the way back", () => {
    expect((h.match(/<figure class="diff"/g) ?? []).length).toBe(2);
    expect(h).toContain('aria-label="Exact changes to C:\\Users\\Home Office\\.claude\\settings.json"');
    expect(h).toContain('aria-label="Exact changes to /home/rotem/.codex/hooks.json"');
    expect(h).toContain("A backup is saved first");
    expect(h).toContain("puts that backup back, for as long as the files are unchanged since this fix");
  });
  it("skips a file with no diff and lists a deleted file", () => {
    const plan = { ...DOCTOR_PLAN, preview: { ...DOCTOR_PREVIEW, files: [filePreview("C:\\a.json", "", true), DOCTOR_PREVIEW.files[0]!] } };
    const out = sheet({ plan });
    expect((out.match(/<figure class="diff"/g) ?? []).length).toBe(1);
    expect(out).toContain("Deletes");
    expect(out).toContain("C:\\a.json");
  });
  it("names the restart count, and makes the confirm button the headline", () => {
    expect(h).toContain("2 sessions pick this up after a restart");
    expect(h).toContain(">Fix 3 issues<");
    expect(focusOrder(h)).toEqual(["Changes to review", "Cancel", "Fix 3 issues"]);
  });
  it("is a dialog labelled by its title and takes focus on the title", () => {
    expect(h).toContain('role="dialog"');
    expect(h).toContain('tabindex="-1"');
    expect(h).not.toMatch(/autofocus/i);
  });
  it("busy: the confirm button says Fixing… and is aria-disabled", () => {
    expect(sheet({ busy: true })).toMatch(/aria-disabled="true"[^>]*>Fixing…</);
  });
  it("conflict: says nothing was written, offers Reload and Cancel, and no way to apply", () => {
    const c = sheet({ conflict: true });
    expect(c).toContain("A file changed after this preview was made, so nothing was written. Reload to see what it says now.");
    expect(c).toContain(">Reload<");
    expect(c).toContain(">Cancel<");
    expect(c).not.toContain("Fix 3 issues</button>");
    expect(c).not.toMatch(/Allow|data-primary>Fix/);
  });
});

describe("DifferencesSheet", () => {
  const h = renderToStaticMarkup(<DifferencesSheet preview={DOCTOR_PREVIEW} onClose={noop} />);
  it("shows a diff per file and a Close button", () => {
    expect((h.match(/<figure class="diff"/g) ?? []).length).toBe(2);
    expect(h).toContain('role="dialog"');
    expect(h).toContain(">Close<");
  });
});
