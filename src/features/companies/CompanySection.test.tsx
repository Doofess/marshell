import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import type { AgentInstall } from "../../generated/AgentInstall";
import type { AgentSettingsView } from "../../generated/AgentSettingsView";
import { CompanyView } from "./CompanySection";

const install = (over: Partial<AgentInstall> = {}): AgentInstall => ({ agent: "claude", found: true, version: "2.1.0", install: { kind: "native" }, update_command: "claude update", newer_version: null, ...over });
const view = (over: Partial<AgentSettingsView> = {}): AgentSettingsView => ({
  agent: "claude",
  path: "C:/u/.claude/settings.json",
  state: { kind: "ok" },
  curated: [
    { key: "model", value: "opus", scope: "user" },
    { key: "effortLevel", value: null, scope: "user" },
    { key: "theme", value: "dark", scope: "managed" },
  ],
  others: [],
  raw: '{\n  "model": "opus"\n}',
  ...over,
});
const render = (over: Partial<Parameters<typeof CompanyView>[0]> = {}) =>
  renderToStaticMarkup(<CompanyView agent="claude" name="Claude Code" install={install()} view={view()} running={0} mode="form" onMode={() => {}} onUpdate={() => {}} {...over} />);

describe("CompanyView", () => {
  it("shows the version and an Update button when there is an update command", () => {
    const html = render();
    expect(html).toContain("Version 2.1.0");
    expect(html).toContain("Update Claude Code");
  });
  it("shows the manual line instead for an unknown install", () => {
    const html = render({ install: install({ update_command: null, install: { kind: "unknown" } }) });
    expect(html).not.toContain("Update Claude Code");
    expect(html).toContain("Marshell can&#x27;t tell how Claude Code was installed.");
    expect(html).toContain("<code>claude update</code>");
  });
  it("shows the newer version and the running note", () => {
    const html = render({ install: install({ newer_version: "2.2.0" }), running: 2 });
    expect(html).toContain("Version 2.2.0 is available.");
    expect(html).toContain("Claude Code is running in 2 tabs. The update may fail while they run.");
  });
  it("shows an en dash for an unset curated key and the Managed badge", () => {
    const html = render();
    expect(html).toContain("Default model");
    expect(html).toContain("opus");
    expect(html).toContain("–");
    expect(html).toContain('data-scope="managed"');
    expect(html).toContain('data-scope="user"');
  });
  it("shows Other settings only when there are some", () => {
    expect(render()).not.toContain("Other settings in this file");
    const html = render({ view: view({ others: [{ key: "alwaysThinking", value: "true", scope: "user" }] }) });
    expect(html).toContain("<h3");
    expect(html).toContain("Other settings in this file");
    expect(html).toMatch(/<code[^>]*>alwaysThinking<\/code>/);
  });
  it("disables Raw file when raw is null", () => {
    expect(render({ view: view({ raw: null }) })).toMatch(/<input(?=[^>]*value="raw")(?=[^>]*disabled)/);
    expect(render()).not.toMatch(/<input(?=[^>]*value="raw")(?=[^>]*disabled)/);
  });
  it("shows the raw note and a read-only text area in raw mode", () => {
    const html = render({ mode: "raw" });
    expect(html).toContain("Shown as Marshell reads it: comments and layout are left out, and secret values show as hidden.");
    expect(html).toMatch(/<textarea[^>]*readOnly=""/);
  });
  it("has no input but the two radios and no enabled textarea", () => {
    for (const mode of ["form", "raw"] as const) {
      const html = render({ mode, view: view({ others: [{ key: "x", value: "1", scope: "user" }] }) });
      const inputs = html.match(/<input[^>]*>/g) ?? [];
      expect(inputs).toHaveLength(2);
      for (const i of inputs) expect(i).toContain('type="radio"');
      for (const t of html.match(/<textarea[^>]*>/g) ?? []) expect(t).toContain('readOnly=""');
    }
  });
  it("shows the file line", () => {
    expect(render({ view: view({ state: { kind: "missing" } }) })).toContain("doesn&#x27;t exist yet");
  });
  it("still renders the heading while nothing has loaded", () => {
    expect(render({ install: null, view: null })).toContain("Claude Code");
  });
});
