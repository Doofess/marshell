import { renderToStaticMarkup } from "react-dom/server";
import { createElement } from "react";
import { describe, expect, it } from "vitest";
import { parseUnifiedDiff } from "../../components/DiffView/diff";
import { ConsentCard } from "./ConsentCard";
import { Detected } from "./Detected";
import { canEdit, consentHeadline, entryCount, type Capability, type Detection } from "./consent";
import { emptyStatesFor } from "./emptyStates";
import { CONSENT, DETECTIONS } from "./fixtures";

const ALL: Record<Capability, boolean> = { status: true, approve: true, statusline: true };
const added = (diff: string) => parseUnifiedDiff(diff).lines.filter((l) => l.kind === "add").map((l) => l.text);
const entriesIn = (diff: string) => added(diff).filter((t) => /(?:event|permission) --agent/.test(t)).length;

describe("every consent headline tells the truth about its diff", () => {
  for (const [name, s] of Object.entries(CONSENT)) {
    const d = s.detection;
    if (!canEdit(d).ok || s.plan.headline) continue;
    it(`${name}: the count in the headline is the number of hook entries the diff adds`, () => {
      const on = name === "gemini" ? { status: true, approve: false, statusline: false } : ALL;
      expect(entriesIn(s.plan.diff)).toBe(entryCount(d.cli, on));
      expect(consentHeadline(d, on)).toContain(`Add ${entryCount(d.cli, on)} hook entries`);
    });
  }
  it("a stand-in change used to show a long diff says so in its own headline", () => {
    const s = CONSENT.codexLong;
    expect(s.plan.headline).toMatch(/stand-in/i);
    expect(entriesIn(s.plan.diff)).toBeGreaterThan(30);
    expect(s.plan.headline).toContain(`${entriesIn(s.plan.diff)}`);
    expect(parseUnifiedDiff(s.plan.diff).lines.length).toBeGreaterThan(150);
  });
});

describe("the bridge path has a space in it, so every command in every diff quotes it", () => {
  for (const [name, s] of [...Object.entries(CONSENT)]) {
    it(`${name}: no added line runs the bridge from an unquoted path`, () => {
      const lines = added(s.plan.diff).filter((t) => t.includes("marshell-hook"));
      for (const t of lines) expect(t, t).toMatch(/(?:\\"|")C:\/Users\/Home Office\/\.marshell\/bin\/marshell-hook\.exe(?:\\"|")/);
    });
  }
  it("the Codex command string keeps its quoted path inside the TOML literal", () => {
    expect(added(CONSENT.codex.plan.diff).some((t) => t.startsWith("command = '\"C:/Users/Home Office/.marshell/bin/marshell-hook.exe\" event --agent codex"))).toBe(true);
  });
});

describe("chaining the status line keeps the user's own command visible", () => {
  it("the Claude diff shows where the original is kept, as an added file", () => {
    const lines = added(CONSENT.claude.plan.diff);
    expect(lines.some((t) => t.includes("python ~/.claude/statusline.py"))).toBe(true);
    expect(parseUnifiedDiff(CONSENT.claude.plan.diff).files.some((f) => f.endsWith("statusline-chain.json"))).toBe(true);
  });
  it("the card says the original keeps running and names the file", () => {
    const h = renderToStaticMarkup(createElement(ConsentCard, { scenario: CONSENT.claude, on: ALL }));
    expect(h).toContain("statusline-chain.json");
    expect(h).toContain("keeps running");
  });
});

describe("undo copy only names things that exist, and promises only what holds", () => {
  it("never names a settings page that does not exist, or a byte-exact restore of an uninstall", () => {
    for (const s of Object.values(CONSENT)) {
      expect(s.plan.removal).not.toMatch(/byte for byte|Remove all hooks|Advanced/);
      expect(s.plan.removal).toMatch(/only the entries Marshell added|only what Marshell added/i);
    }
  });
});

describe("a file that cannot be written is never described as about to change", () => {
  const readOnly = DETECTIONS.readOnly[1]!;
  it("says the file is read-only instead of 'Add 7 hook entries'", () => {
    const text = consentHeadline(readOnly, ALL);
    expect(text).toBe(`${readOnly.configPath} is read-only, so Marshell will not edit it.`);
    expect(text).not.toMatch(/^Add /);
  });
  it("the read-only card has no switches and gives the reason", () => {
    const h = renderToStaticMarkup(createElement(ConsentCard, { scenario: CONSENT.readOnlyCodex, on: ALL }));
    expect(h).toContain("is read-only, so Marshell will not edit it.");
    expect(h).toContain("The file is read-only.");
    expect(h).not.toContain('role="switch"');
    expect(h).not.toContain("View exact changes");
  });
});

describe("an unknown value is an en dash, never a guess", () => {
  const noLine: Detection = { ...DETECTIONS.unparsable[2]!, parseError: undefined };
  it("the headline and the detected row do not say line 0", () => {
    expect(consentHeadline(noLine, ALL)).toContain("(line –)");
    expect(consentHeadline(noLine, ALL)).not.toContain("line 0");
    const h = renderToStaticMarkup(createElement(Detected, { detections: [noLine] }));
    expect(h).toContain("line –");
    expect(h).not.toContain("line 0");
  });
});

describe("headline and card wording", () => {
  it("the Gemini headline names the setting it turns on", () => {
    const d = DETECTIONS.allThree[2]!;
    expect(consentHeadline(d, { status: true, approve: false, statusline: false })).toContain("hooksConfig.enabled");
  });
  it("the Codex trust sentence says /hooks once", () => {
    const h = renderToStaticMarkup(createElement(ConsentCard, { scenario: CONSENT.codex, on: ALL }));
    const trust = /<p class="consent__trust">(.*?)<\/p>/.exec(h)![1]!;
    expect(trust.match(/\/hooks/g)).toHaveLength(1);
  });
});

describe("empty states take their shortcut from the keymap", () => {
  it("shows the launcher's own binding per system and not a hard-coded one", () => {
    const win = emptyStatesFor("windows").find((e) => e.id === "no-sessions")!;
    const mac = emptyStatesFor("mac").find((e) => e.id === "no-sessions")!;
    expect(win.body).toBe("Start one with Ctrl+Shift+T.");
    expect(mac.body).not.toContain("Ctrl");
    expect(emptyStatesFor("windows").find((e) => e.id === "departures-empty")!.body).toBe("Start one with Ctrl+Shift+T.");
  });
});
