import { renderToStaticMarkup } from "react-dom/server";
import { createElement } from "react";
import { describe, expect, it } from "vitest";
import { parseUnifiedDiff } from "../../components/DiffView/diff";
import { ConsentCard } from "./ConsentCard";
import { Detected } from "./Detected";
import { canEdit, CAPABILITIES, consentHeadline, entryCount, removalCopy, type Capability, type Detection } from "./consent";
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
      const n = entryCount(d.cli, ALL);
      expect(entriesIn(s.plan.diff)).toBe(n);
      // A CLI with no hooks to add says so: its headline never says "Add".
      if (n > 0) expect(consentHeadline(d, ALL)).toContain(`Add ${n} hook entries`);
      else expect(consentHeadline(d, ALL)).not.toMatch(/^Add /);
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
});

describe("the Codex diff writes hooks.json in the shape Codex documents", () => {
  // docs/research/codex-cli.md and Task 1's recording: `{"hooks": {"<Event>": [{"hooks": [{"type": "command", "command", "timeout"}]}]}}`, no matcher.
  const EVENTS = ["SessionStart", "UserPromptSubmit", "PreToolUse", "PostToolUse", "Stop", "Interrupt", "SessionEnd", "PermissionRequest"];
  type Handler = { type: string; command: string; timeout: number };
  type File = { hooks: Record<string, Array<{ matcher?: string; hooks: Handler[] }>> };
  /** The file the diff creates, read back from its added lines. */
  const fileOf = (diff: string): File => JSON.parse(added(diff).join("\n")) as File;
  const codex = fileOf(CONSENT.codex.plan.diff);
  const only = (event: string): Handler => {
    const groups = codex.hooks[event]!;
    expect(groups, event).toHaveLength(1);
    expect(groups[0]!.matcher, event).toBeUndefined();
    expect(groups[0]!.hooks, event).toHaveLength(1);
    return groups[0]!.hooks[0]!;
  };

  it("one group per event, each with one command handler and no matcher", () => {
    expect(Object.keys(codex.hooks)).toEqual(EVENTS);
    for (const e of EVENTS) expect(only(e).type).toBe("command");
  });
  it("the Codex entry count matches its diff: the headline count is the number of events written, split as the capabilities say", () => {
    expect(Object.keys(codex.hooks)).toHaveLength(entryCount("codex", ALL));
    expect(consentHeadline(CONSENT.codex.detection, ALL)).toContain(`Add ${Object.keys(codex.hooks).length} hook entries`);
    const roles = EVENTS.map((e) => / (event|permission) --agent codex/.exec(only(e).command)?.[1]);
    expect(roles.filter((r) => r === "event")).toHaveLength(CAPABILITIES.codex.status.entries);
    expect(roles.filter((r) => r === "permission")).toHaveLength(CAPABILITIES.codex.approve.entries);
    expect(only("PermissionRequest").command).toContain(" permission --agent codex");
  });
  it("every command runs the bridge through PowerShell's call operator, quoted, and names the agent and the tag", () => {
    for (const e of EVENTS) expect(only(e).command, e).toMatch(/^& "C:\/Users\/Home Office\/\.marshell\/bin\/marshell-hook\.exe" (?:event|permission) --agent codex --tag mshl1$/);
  });
  it("timeouts are explicit seconds: 5 for telemetry, 2 for SessionEnd and Interrupt, 600 for the permission decision", () => {
    for (const e of EVENTS) expect(only(e).timeout, e).toBe(e === "PermissionRequest" ? 600 : e === "SessionEnd" || e === "Interrupt" ? 2 : 5);
  });
  it("the stand-in long diff keeps the shape and stays labelled as a stand-in", () => {
    const long = fileOf(CONSENT.codexLong.plan.diff);
    expect(Object.keys(long.hooks).length).toBeGreaterThan(30);
    for (const groups of Object.values(long.hooks)) expect(groups[0]!.hooks).toHaveLength(1);
    expect(CONSENT.codexLong.plan.headline).toMatch(/stand-in/i);
    expect(CONSENT.codexLong.plan.headline).toContain(`Add ${Object.keys(long.hooks).length} hook entries`);
  });
  it("the source line names hooks.json, not config.toml, and the trust line says /hooks once", () => {
    const h = renderToStaticMarkup(createElement(ConsentCard, { scenario: CONSENT.codex, on: ALL }));
    expect(h).toContain(
      '<p class="consent__source">The hook entries go in <code>hooks.json</code> only, not in <code>config.toml</code> as well: Codex reads both and would warn about duplicates.</p>',
    );
    expect(h).toContain(
      '<p class="consent__trust">Codex runs hooks only after you trust them. The next time Codex starts, it lists them as new: choose <strong>Trust all and continue</strong>, or review them first in <code>/hooks</code>.</p>',
    );
    expect(/<p class="consent__trust">(.*?)<\/p>/.exec(h)![1]!.match(/\/hooks/g)).toHaveLength(1);
    // The note is Codex's alone: Claude's card never mentions hooks.json, and Antigravity's names it only as the file it writes.
    expect(renderToStaticMarkup(createElement(ConsentCard, { scenario: CONSENT.claude, on: ALL }))).not.toContain("hooks.json");
    expect(renderToStaticMarkup(createElement(ConsentCard, { scenario: CONSENT.antigravity, on: ALL }))).not.toContain('class="consent__source"');
  });
  it("writes no config.toml", () => {
    expect(CONSENT.codex.plan.diff).not.toContain("config.toml");
    expect(CONSENT.codex.detection.configPath).toMatch(/hooks\.json$/);
  });
});

describe("the Antigravity diff writes ~/.gemini/config/hooks.json in the shape its bundled docs give", () => {
  // docs/research/2026-10-15-local-install-check.md, "Hooks": one named hook with `enabled`; PreToolUse and PostToolUse are matcher
  // groups, PreInvocation, PostInvocation and Stop are flat handler lists; `timeout` is seconds and defaults to 30.
  const HOOKS_PATH = "C:\\Users\\Home Office\\.gemini\\config\\hooks.json";
  /** One file's part of a multi-file diff: from its `---` line up to the next file's. */
  const section = (path: string): string => {
    const text = CONSENT.antigravity.plan.diff;
    const at = text.indexOf(`+++ b/${path}`);
    expect(at, path).toBeGreaterThan(-1);
    const end = text.indexOf("\n--- ", at);
    return text.slice(text.lastIndexOf("--- ", at), end === -1 ? undefined : end + 1);
  };
  /** The new hooks.json, read back from the diff: the added lines of that file, in order. */
  const hooksJson = (): Record<string, Record<string, unknown>> => JSON.parse(added(section(HOOKS_PATH)).join("\n"));
  type Handler = { type: string; command: string; timeout: number };
  const handlers = (event: string): Handler[] => {
    const v = hooksJson().marshell![event] as Array<Handler | { matcher: string; hooks: Handler[] }>;
    return v.flatMap((x) => ("hooks" in x ? x.hooks : [x]));
  };

  it("is a new file holding one named hook, marshell, that is on", () => {
    expect(parseUnifiedDiff(CONSENT.antigravity.plan.diff).files).toContain(HOOKS_PATH);
    expect(section(HOOKS_PATH)).toMatch(/^--- \/dev\/null$/m);
    expect(Object.keys(hooksJson())).toEqual(["marshell"]);
    expect(hooksJson().marshell!.enabled).toBe(true);
  });
  it("groups the tool events by a matcher that takes every tool, and lists the other three flat", () => {
    const m = hooksJson().marshell!;
    for (const e of ["PreToolUse", "PostToolUse"]) expect(m[e], e).toEqual([{ matcher: ".*", hooks: expect.any(Array) }]);
    for (const e of ["PreInvocation", "PostInvocation", "Stop"]) for (const h of m[e] as Handler[]) expect(h.type, e).toBe("command");
  });
  it("one command per event, and each names its event: Antigravity's payload does not say which event fired", () => {
    const events = ["PreToolUse", "PostToolUse", "PreInvocation", "PostInvocation", "Stop"];
    expect(Object.keys(hooksJson().marshell!).filter((k) => k !== "enabled")).toEqual(events);
    for (const e of events) {
      expect(handlers(e), e).toHaveLength(1);
      expect(handlers(e)[0]!.command).toMatch(new RegExp(`^"C:/Users/Home Office/\\.marshell/bin/marshell-hook\\.exe" (?:event|permission) --agent antigravity --event ${e} --tag mshl1$`));
    }
  });
  it("PreToolUse is the permission hook with a 600 s timeout (the default 30 would cut a person off); the rest are events capped at 10 s", () => {
    expect(handlers("PreToolUse")[0]).toMatchObject({ command: expect.stringContaining(" permission --agent antigravity"), timeout: 600 });
    for (const e of ["PostToolUse", "PreInvocation", "PostInvocation", "Stop"]) expect(handlers(e)[0], e).toMatchObject({ command: expect.stringContaining(" event --agent antigravity"), timeout: 10 });
  });
  it("with the mode handed over, the settings diff removes dangerouslySkipPermissions and nothing else of the user's", () => {
    const s = CONSENT.antigravityHandOver;
    const lines = parseUnifiedDiff(s.plan.diff).lines;
    expect(lines.filter((l) => l.kind === "del").map((l) => l.text.trim())).toEqual(['"dangerouslySkipPermissions": true,', '"statusLine": { "type": "command", "command": "~/.gemini/antigravity-cli/statusline.sh" }']);
    expect(lines.some((l) => l.kind === "add" && /dangerouslySkipPermissions/.test(l.text))).toBe(false);
    expect(consentHeadline(s.detection, ALL, true)).toBe(
      "Add 5 hook entries in C:\\Users\\Home Office\\.gemini\\config\\hooks.json (a new file). In C:\\Users\\Home Office\\.gemini\\antigravity-cli\\settings.json, chain your status line and turn off dangerouslySkipPermissions.",
    );
    expect(consentHeadline(s.detection, { status: false, approve: false, statusline: false }, true)).toBe(
      "Turn off dangerouslySkipPermissions in C:\\Users\\Home Office\\.gemini\\antigravity-cli\\settings.json.",
    );
    expect(s.plan.removal).toBe(
      "Marshell removes the hook entries it added, deletes hooks.json if nothing else is in it, puts your own status line back from statusline-chain.json, and turns dangerouslySkipPermissions back on; the rest of each file stays as it is.",
    );
  });
  it("puts no hooks in settings.json: Antigravity reads them only from hooks.json", () => {
    expect(section("C:\\Users\\Home Office\\.gemini\\antigravity-cli\\settings.json")).not.toMatch(/"hooks"|PreToolUse|hooksConfig/);
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
  it("the Antigravity diff chains the status line in ~/.gemini/antigravity-cli/settings.json and keeps the user's own command", () => {
    const s = CONSENT.antigravity;
    expect(s.detection.configPath).toBe("C:\\Users\\Home Office\\.gemini\\antigravity-cli\\settings.json");
    const lines = added(s.plan.diff);
    expect(lines.some((t) => t.includes("statusline --agent antigravity"))).toBe(true);
    expect(lines.some((t) => t.includes("~/.gemini/antigravity-cli/statusline.sh"))).toBe(true);
    expect(parseUnifiedDiff(s.plan.diff).files.some((f) => f.endsWith("agents\\antigravity\\statusline-chain.json"))).toBe(true);
    const h = renderToStaticMarkup(createElement(ConsentCard, { scenario: s, on: s.plan.on }));
    expect(h).toContain("antigravity\\statusline-chain.json");
    expect(h).toContain("keeps running");
  });
});

describe("undo copy says what removal does for each CLI, and promises nothing more", () => {
  it("Claude: removes the hook entries and puts the user's own status line back from the chain file", () => {
    expect(CONSENT.claude.plan.removal).toBe("Marshell removes the hook entries it added and puts your own status line back from statusline-chain.json; the rest of the file stays as it is.");
  });
  it("Codex: removes its hook entries, and says nothing about a status line it never chained", () => {
    expect(CONSENT.codex.plan.removal).toBe("Marshell removes the hook entries it added; the rest of the file stays as it is.");
    expect(CONSENT.codex.plan.removal).not.toContain("status line");
  });
  it("Antigravity: removes its hook from the hooks.json it created (deleting the file only if nothing else is in it) and puts the status line back", () => {
    expect(CONSENT.antigravity.plan.removal).toBe(
      "Marshell removes the hook entries it added, deletes hooks.json if nothing else is in it, and puts your own status line back from statusline-chain.json; the rest of each file stays as it is.",
    );
    expect(removalCopy("antigravity", { status: false, approve: false, statusline: true }, { hooksFile: "hooks.json", created: true })).toBe(
      "Marshell puts your own status line back from statusline-chain.json; the rest of the file stays as it is.",
    );
    expect(removalCopy("antigravity", { status: true, approve: true, statusline: false }, { hooksFile: "hooks.json", created: false })).toBe(
      "Marshell removes the hook entries it added; the rest of the file stays as it is.",
    );
  });
  it("follows the switches: with the status line off, Claude's undo does not mention the chain file", () => {
    expect(removalCopy("claude", { status: true, approve: true, statusline: false })).toBe("Marshell removes the hook entries it added; the rest of the file stays as it is.");
    expect(removalCopy("claude", { status: false, approve: false, statusline: false })).toBe("Marshell changes nothing, so there is nothing to undo.");
  });
  it("never claims that only added entries are removed when a status line command was replaced, and never names a page that does not exist", () => {
    for (const s of Object.values(CONSENT)) expect(s.plan.removal).not.toMatch(/byte for byte|Remove all hooks|Advanced|removes only the entries/);
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
  it("the Antigravity headline names each file with what goes in it, and no Gemini-era setting", () => {
    const d = DETECTIONS.allThree[2]!;
    expect(d.cli).toBe("antigravity");
    const hooks = "C:\\Users\\Home Office\\.gemini\\config\\hooks.json";
    expect(consentHeadline(d, ALL)).toBe(`Add 5 hook entries in ${hooks} (a new file) and chain your status line in ${d.configPath}.`);
    expect(consentHeadline(d, { status: true, approve: true, statusline: false })).toBe(`Add 5 hook entries in ${hooks} (a new file).`);
    expect(consentHeadline(d, { status: false, approve: false, statusline: true })).toBe(`Chain your status line in ${d.configPath}.`);
    expect(consentHeadline({ ...d, hooks: { path: hooks, exists: true } }, { status: true, approve: false, statusline: false })).toBe(`Add 4 hook entries in ${hooks}.`);
    expect(consentHeadline(d, { status: false, approve: false, statusline: false })).toBe("Change nothing. Antigravity CLI will show limited status.");
    expect(consentHeadline(d, ALL)).not.toMatch(/hooksConfig|Gemini/);
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
