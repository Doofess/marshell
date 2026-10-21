import { readdirSync, readFileSync, statSync } from "node:fs";
import { join, relative } from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";
import { rules } from "./cssRules";

/**
 * The accessibility safety net's lint layer (docs/PLAN.md, "Accessibility safety net"). Two rules that kept being found late
 * by review, now enforced for the whole of src/:
 *   1. The app has ONE live region, the Announcer. Anything else that announces needs a reason written here.
 *   2. Every selected state that is drawn with a fill or a shadow has a forced-colors rule in the same file, because
 *      Windows High Contrast drops fills and shadows and the selection would vanish.
 */
const SRC = fileURLToPath(new URL("../../", import.meta.url));

function walk(dir: string, out: string[] = []): string[] {
  for (const name of readdirSync(dir)) {
    const p = join(dir, name);
    if (statSync(p).isDirectory()) walk(p, out);
    else out.push(p);
  }
  return out;
}
const files = walk(SRC);
const rel = (p: string) => relative(SRC, p).split("\\").join("/");
const isSource = (p: string) => /\.(ts|tsx)$/.test(p) && !/\.(test|stories)\.(ts|tsx)$/.test(p);

const LIVE = /role=["{]*["']?(status|alert)\b|aria-live=/;

/**
 * Files allowed to contain a live region, and why. `migrate` entries are debts: the file must still contain one (so the entry
 * is removed the moment it is paid) and the list of debts may only shrink.
 */
const ALLOWED: Record<string, { reason: string; migrate?: true }> = {
  "components/a11y/Announcer.tsx": { reason: "THE live region of the app" },
  "features/launcher/Launcher.tsx": { reason: "'No folder matches' while typing, and the task confirm's single progress line ('Creating the worktree…', then 'Starting <program>…'), in a modal sheet that owns focus; the window's announcer is quiet while it is open" },
  "features/onboarding/ConsentCard.tsx": { reason: "the error from an Allow press (the file could not be written) appears beside the button that has focus and is not announced anywhere else; it exists only after a failed attempt" },
  "features/notifications/NotificationsDialog.tsx": { reason: "the dialog's read and save failure alert, in a modal sheet that owns focus; it exists only after a failed request and the window's announcer is quiet while it is open" },
  "features/agents/AgentsDialog.tsx": { reason: "the dialog's read and refusal alerts, in a modal sheet that owns focus; they exist only after a failed request and the window's announcer is quiet while it is open" },
  "features/diagnostics/DiagnosticsDialog.tsx": { reason: "the dialog's preparing status and read and refusal alerts, in a modal sheet that owns focus; they exist only after a request and the window's announcer is quiet while it is open" },
  "features/onboarding/LiveOnboarding.tsx": { reason: "'Couldn't read the setup', in the first-run screen that owns focus; it exists only after a failed read and the window's announcer is quiet while it is up" },
  "features/palette/Palette.tsx": { reason: "'No command matches' while typing in a modal sheet that owns focus; the window's announcer is quiet while it is open" },
  "features/tasks/ReviewSheet.tsx": { reason: "the merge step's single progress line ('Merging…', 'Removing…') and its error alerts, in a modal sheet that owns focus; the window's announcer is quiet while it is open" },
  "features/sessions/LiveApp.tsx": { reason: "the startup failure screen: there is no window yet, so no announcer" },
  "features/sessions/CloseBar.tsx": { reason: "the busy-close question owns keyboard focus while it is up and exists only then; it is not a status message, and the window's Announcer cannot say a question that is still open" },
  "features/sessions/PauseBar.tsx": { reason: "the Pause questions are the close bar's twin: the bar owns keyboard focus while it is up and exists only then; it is not a status message, and the window's Announcer cannot say a question that is still open" },
  "features/sessions/RestartBar.tsx": { reason: "the restart question is the close bar's twin: it owns keyboard focus while it is up and exists only then; it is not a status message, and the window's Announcer cannot say a question that is still open" },
  "features/preview/Preview.tsx": { reason: "the header's 'Updated' note, polite only while focus is inside the preview (someone reading the file) and off otherwise, so a refresh never interrupts a person typing in the terminal; the window's announcer would say it to everyone" },
  "features/regions/RegionLayer.tsx": { reason: "the marked box's size and place, said 300 ms after the last arrow key so a run of presses is one sentence; the box owns keyboard focus while it is up and the window's Announcer cannot follow a box being moved" },
  "features/regions/RegionComment.tsx": { reason: "the reason a region could not be added (it is paused, no session), in a popover that owns keyboard focus while it is up; it is not a status message the window's Announcer could keep" },
  "features/perf/PerfOverlay.tsx": { reason: 'aria-live="off": a developer overlay that explicitly stays silent' },
  "features/window/NavCursor.tsx": { reason: "migrate: the navigation cursor's own region says 'Selected session 6' and must go through the Announcer so the window has one", migrate: true },
};

describe("shared blocks: one live region", () => {
  const names = files.filter(isSource).filter((p) => LIVE.test(readFileSync(p, "utf8"))).map(rel);
  it("approval and lane features use the shared announcer for every outcome", () => {
    const regions = names.filter((n) => /^features\/(approval|lane)\//.test(n));
    expect(regions).toEqual([]);
  });

  it("only the allowed files contain a live region (role=status, role=alert or aria-live)", () => {
    const unexpected = names.filter((n) => !(n in ALLOWED));
    expect(unexpected, `Say it through the Announcer (src/components/a11y/Announcer.tsx) or add the file to ALLOWED with a reason: ${unexpected.join(", ")}`).toEqual([]);
  });
  it("every allowed file still has one, so an entry is removed when its region is", () => {
    const stale = Object.keys(ALLOWED).filter((n) => !names.includes(n));
    expect(stale, `Remove from ALLOWED: ${stale.join(", ")}`).toEqual([]);
  });
  it("lists the migration debts, and there are no new ones", () => {
    expect(Object.entries(ALLOWED).filter(([, v]) => v.migrate).map(([k]) => k)).toEqual(["features/window/NavCursor.tsx"]);
  });
  it("every entry says why", () => {
    for (const [file, v] of Object.entries(ALLOWED)) expect(v.reason.length, file).toBeGreaterThan(20);
  });
});

/** A selected-state selector: aria-selected, aria-current or data-selected set to true (a :not(...) of one is the opposite and does not count). */
const SELECTED = /\[(aria-selected|aria-current|data-selected)="true"\]/;
const withoutNegations = (s: string) => s.replace(/:not\([^)]*\)/g, "");

describe("shared blocks: a selected state survives Windows High Contrast", () => {
  const css = files.filter((p) => p.endsWith(".css"));
  for (const file of css) {
    const rs = rules(readFileSync(file, "utf8"));
    const selected = [...new Set(rs.filter((r) => !r.forced).flatMap((r) => r.selectors).filter((s) => SELECTED.test(withoutNegations(s))))];
    if (selected.length === 0) continue;
    it(`${rel(file)}: every selected-state selector has a forced-colors rule`, () => {
      const forced = new Set(rs.filter((r) => r.forced).flatMap((r) => r.selectors));
      const missing = selected.filter((s) => !forced.has(s));
      expect(missing, `Add a forced-colors rule (an outline in Highlight) for: ${missing.join(" | ")}. Or use the shared .is-selected-mark (src/components/a11y/SelectedMark.css).`).toEqual([]);
    });
  }
  it("finds the selected rules it is meant to guard (the scan is not empty)", () => {
    const all = css.flatMap((f) => rules(readFileSync(f, "utf8"))).flatMap((r) => r.selectors);
    expect(all.filter((s) => SELECTED.test(withoutNegations(s))).length).toBeGreaterThan(5);
  });
});
