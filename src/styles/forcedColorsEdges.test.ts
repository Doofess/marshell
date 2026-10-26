import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";

/**
 * Windows High Contrast removes box-shadows and background fills, so a control told apart only by a fill would
 * lose its edge. Each one gets a 1 px outline in system colours inside a forced-colors block.
 */
const EDGES: Record<string, string[]> = {
  "features/approval/ApproveCard.css": [".approve-card", ".approve-card__btn"],
  "components/Chip/Chip.css": [".chip"],
  "components/DiffView/DiffView.css": [".diff__more"],
  "components/EmptyState/EmptyState.css": [".empty-state__action"],
  "components/Switch/Switch.css": [".switch__track"],
  "features/settings/Settings.css": [".scope-badge", ".field", ".settings__banner", ".settings__btn", ".settings__nav-item", ".change-bar", ".field__control", ".raw-editor__text"],
  "features/doctor/Doctor.css": [".doctor__row", ".doctor__failed", ".doctor__btn", ".doctor__link", ".toast", ".toast__btn", ".fix-sheet", ".fix-sheet__btn"],
  "features/sessions/CloseBar.css": [".close-bar__btn"],
  "features/onboarding/Onboarding.css": [".consent", ".detected__row", ".consent__toggle", ".consent__skip"],
  "features/departures/Board.css": [".board__status", ".board__waiting", ".board__all"],
  "features/notifications/OsBanner.css": [".os-banner", ".os-banner__btn"],
  "features/tray/Tray.css": [".tray-menu"],
  "features/window/SplitView.css": [".pane__new", ".pane__choice"],
  "features/window/MainWindow.css": [".window", ".window__palette", ".focus-pill"],
  "features/window/LayoutMenu.css": [".layout-menu", ".layout-menu__option", ".layout-menu__count"],
  "features/composer/Composer.css": [".composer", ".composer__send"],
  "features/composer/StatusBar.css": [".status-bar"],
  "features/window/Rail.css": [".rail__tool"],
  "features/launcher/Launcher.css": [".launcher__sheet", ".launcher__chip", ".launcher__input", ".launcher__start", ".launcher__program"],
  "features/palette/Palette.css": [".palette__sheet", ".palette__input"],
  "features/notifications/NotificationsSection.css": [".notifications-dialog__sheet", ".notifications-dialog__group", ".notifications-dialog__input", ".notifications-dialog__btn"],
  "features/agents/AgentsSection.css": [".agents-dialog__sheet", ".agents-dialog__btn", ".agents-dialog__warning"],
};

const read = (f: string) => readFileSync(new URL(`../${f}`, import.meta.url), "utf8").replace(/\/\*[\s\S]*?\*\//g, "");

/** The bodies of every `@media (forced-colors: active) { … }` block, found by matching braces. */
function forcedColorsBlocks(css: string): string[] {
  const out: string[] = [];
  let from = 0;
  for (;;) {
    const i = css.indexOf("@media (forced-colors: active)", from);
    if (i < 0) return out;
    const open = css.indexOf("{", i);
    let depth = 0;
    let j = open;
    for (; j < css.length; j++) {
      if (css[j] === "{") depth++;
      else if (css[j] === "}" && --depth === 0) break;
    }
    out.push(css.slice(open + 1, j));
    from = j;
  }
}

/** True when `selector` is in a selector list whose declarations include the outline. */
function hasOutline(block: string, selector: string): boolean {
  for (const m of block.matchAll(/([^{}]+)\{([^{}]*)\}/g)) {
    const selectors = m[1]!.split(",").map((s) => s.trim());
    if (selectors.includes(selector) && /outline:\s*1px solid CanvasText/.test(m[2]!)) return true;
  }
  return false;
}

describe("forced colours: every filled control keeps an edge", () => {
  for (const [file, selectors] of Object.entries(EDGES))
    for (const sel of selectors)
      it(`${file}: ${sel} has a 1px CanvasText outline`, () => {
        const blocks = forcedColorsBlocks(read(file));
        expect(blocks.length, `${file} has a forced-colors block`).toBeGreaterThan(0);
        expect(blocks.some((b) => hasOutline(b, sel))).toBe(true);
      });

  it("never leans on a border colour there (border-color or any side's longhand), since these elements no longer have borders", () => {
    for (const file of Object.keys(EDGES)) for (const b of forcedColorsBlocks(read(file))) expect(b, file).not.toMatch(/border(?:-[a-z]+)*-color\s*:/);
  });
  it("recognises the longhands the rule is about", () => {
    for (const d of ["border-color: Highlight", "border-block-end-color: Highlight", "border-inline-start-color: CanvasText"]) expect(d).toMatch(/border(?:-[a-z]+)*-color\s*:/);
    expect("outline: 1px solid Highlight").not.toMatch(/border(?:-[a-z]+)*-color\s*:/);
  });

  /** A session in a risky mode (bypass, full auto) is a tinted header with a red bar on its start edge; High Contrast drops both, so it takes a 2 px outline. */
  it("features/window/SessionHeader.css: a risky-mode header keeps a 2px CanvasText outline", () => {
    const blocks = forcedColorsBlocks(read("features/window/SessionHeader.css"));
    expect(blocks.some((b) => /\.session-header\[data-danger\]\s*\{[^}]*outline:\s*2px solid CanvasText/.test(b))).toBe(true);
  });
  it("features/window/SplitView.css: a risky-mode pane header keeps the same 2px CanvasText outline", () => {
    const blocks = forcedColorsBlocks(read("features/window/SplitView.css"));
    expect(blocks.some((b) => /\.pane__header\[data-danger\]\s*\{[^}]*outline:\s*2px solid CanvasText/.test(b))).toBe(true);
  });

  /** The selected rail item and the selected session row are marked by an inset fill, which High Contrast drops, so each takes a 2 px Highlight outline. */
  for (const [file, selector] of [
    ["features/window/Rail.css", '.rail__item[aria-current="true"]'],
    // Every visible row is dashed (1px); the focused pane's row is the solid 2px one.
    ["features/sidebar/SessionRow.css", ".session-row[data-focused]"],
  ] as const)
    it(`${file}: ${selector} keeps a 2px Highlight outline`, () => {
      const ok = forcedColorsBlocks(read(file)).some((b) => [...b.matchAll(/([^{}]+)\{([^{}]*)\}/g)].some((m) => m[1]!.split(",").map((x) => x.trim()).includes(selector) && /outline:\s*2px solid Highlight/.test(m[2]!)));
      expect(ok).toBe(true);
    });

  /** Several rows and rail items carry a selection outline, so the keyboard focus takes a third shape, placed after them so it wins. */
  for (const [file, focus, marks] of [
    ["features/window/Rail.css", ".rail__item:focus-visible", ['.rail__item[aria-current="true"]', ".rail__item[data-visible]"]],
    ["features/sidebar/SessionRow.css", ".session-row:focus-visible", ['.session-row[aria-selected="true"]', ".session-row[data-focused]"]],
  ] as const)
    it(`${file}: ${focus} draws a 3px double Highlight outline after the selection outlines`, () => {
      const block = forcedColorsBlocks(read(file)).find((b) => b.includes(focus)) ?? "";
      const rules = [...block.matchAll(/([^{}]+)\{([^{}]*)\}/g)].map((m) => ({ sel: m[1]!.split(",").map((x) => x.trim()), body: m[2]! }));
      const at = rules.findIndex((r) => r.sel.includes(focus));
      expect(at).toBeGreaterThanOrEqual(0);
      expect(rules[at]!.body).toMatch(/outline:\s*3px double Highlight/);
      for (const mark of marks) expect(rules.findIndex((r) => r.sel.includes(mark))).toBeLessThan(at);
    });
});
