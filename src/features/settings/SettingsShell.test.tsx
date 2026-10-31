import { readFileSync } from "node:fs";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { SETTINGS_SECTIONS, shownSections, type SettingsSection } from "./settingsSections";
import { SettingsShell, settingsKey } from "./SettingsShell";

const css = readFileSync(new URL("./SettingsShell.css", import.meta.url), "utf8").replace(/\/\*[\s\S]*?\*\//g, "");
const rules = [...css.matchAll(/([^{}]+)\{([^{}]*)\}/g)].map((m) => ({ selectors: m[1]!.split(",").map((s) => s.trim()), body: m[2]! }));
const rule = (sel: string) => rules.filter((r) => r.selectors.includes(sel)).map((r) => r.body).join(";");

const html = (section: SettingsSection = "terminal") =>
  renderToStaticMarkup(
    <SettingsShell section={section} onSection={() => {}} onClose={() => {}}>
      <p>body</p>
    </SettingsShell>,
  );

describe("SettingsShell", () => {
  it("lists the twelve sections in order", () => {
    expect(SETTINGS_SECTIONS.map((s) => s.label)).toEqual(["Appearance", "Terminal", "Notifications", "Shell profiles", "History", "Projects", "Agents", "Accounts", "Claude Code", "Codex", "Antigravity", "Advanced"]);
    expect(SETTINGS_SECTIONS.map((s) => s.id)).toEqual(["appearance", "terminal", "notifications", "profiles", "history", "projects", "agents", "accounts", "claude", "codex", "antigravity", "advanced"]);
  });
  it("lists a company section only when its CLI is found, and Accounts always", () => {
    const ids = (f: Parameters<typeof shownSections>[0]) => shownSections(f).map((s) => s.id);
    expect(ids(new Set())).toEqual(["appearance", "terminal", "notifications", "profiles", "history", "projects", "agents", "accounts", "advanced"]);
    expect(ids(new Set(["codex"]))).toEqual(["appearance", "terminal", "notifications", "profiles", "history", "projects", "agents", "accounts", "codex", "advanced"]);
    expect(shownSections(new Set(["claude", "codex", "antigravity"]))).toEqual(SETTINGS_SECTIONS);
  });
  it("renders only the sections it is given", () => {
    const h = renderToStaticMarkup(
      <SettingsShell section="codex" sections={[{ id: "codex", label: "Codex" }, { id: "advanced", label: "Advanced" }]} onSection={() => {}} onClose={() => {}}>
        <p>body</p>
      </SettingsShell>,
    );
    const nav = h.match(/<nav[\s\S]*?<\/nav>/)?.[0] ?? "";
    expect([...nav.matchAll(/<button[^>]*>([^<]*)<\/button>/g)].map((m) => m[1])).toEqual(["Codex", "Advanced"]);
    expect(h).toMatch(/<h2[^>]*>Codex<\/h2>/);
  });
  it("has a nav of the twelve labels, and only the current one is marked", () => {
    const h = html();
    const nav = h.match(/<nav[^>]*aria-label="Settings sections"[^>]*>[\s\S]*?<\/nav>/)?.[0] ?? "";
    expect(nav).not.toBe("");
    const labels = [...nav.matchAll(/<button[^>]*>([^<]*)<\/button>/g)].map((m) => m[1]);
    expect(labels).toEqual(SETTINGS_SECTIONS.map((s) => s.label));
    expect(nav.match(/aria-current="page"/g)).toHaveLength(1);
    expect(nav).toMatch(/aria-current="page"[^>]*>Terminal</);
  });
  it("draws the shared selected mark on the current section, which SelectedMark.css keys on aria-current=true or data-selected", () => {
    const nav = html().match(/<nav[\s\S]*?<\/nav>/)?.[0] ?? "";
    const current = nav.match(/<button[^>]*>Terminal</)?.[0] ?? "";
    expect(current).toContain("is-selected-mark");
    expect(current).toContain('data-selected="true"');
    expect(nav.match(/data-selected="true"/g)).toHaveLength(1);
  });
  it("puts the section label in an h2 that can take focus", () => {
    expect(html()).toMatch(/<h2[^>]*tabindex="-1"[^>]*>Terminal<\/h2>/i);
    expect(html("history")).toMatch(/<h2[^>]*>History<\/h2>/);
  });
  it("titles the page Settings and has Back to sessions as a button in the page actions", () => {
    const h = html();
    expect(h).toMatch(/<h1[^>]*>Settings<\/h1>/);
    expect(h).toMatch(/<div class="page__actions"><button[^>]*>Back to sessions<\/button><\/div>/);
  });
  it("renders the children", () => {
    expect(html()).toContain("<p>body</p>");
  });
});

describe("settingsKey", () => {
  it("closes on Escape unless something inside already handled it", () => {
    expect(settingsKey({ key: "Escape", defaultPrevented: false })).toBe("close");
    expect(settingsKey({ key: "Escape", defaultPrevented: true })).toBeNull();
    expect(settingsKey({ key: "a", defaultPrevented: false })).toBeNull();
  });
});

describe("SettingsShell.css", () => {
  it("scrolls nowhere itself but the nav: the page body scrolls the section", () => {
    expect(css.replace(/\.settings-shell__nav\s*\{[^}]*\}/g, "")).not.toMatch(/overflow(?:-[xy])?:\s*(?:auto|scroll|overlay)/);
  });
  it("lets the nav scroll on its own, within the body's height", () => {
    expect(rule(".settings-shell__nav")).toMatch(/overflow-y:\s*auto/);
    expect(rule(".settings-shell__nav")).toMatch(/max-block-size:/);
    expect(rule(".settings-shell__nav")).toMatch(/overscroll-behavior:\s*contain/);
  });
  it("keeps the narrow layout in one column", () => {
    expect(css).toMatch(/@container settings \(inline-size < 720px\)\s*\{[\s\S]*grid-template-columns:\s*minmax\(0,\s*1fr\)/);
  });
  it("has no fixed width over 720px", () => {
    for (const m of css.matchAll(/(?:^|[\s;])(?:inline-size|width|min-inline-size|min-width|max-inline-size|max-width)\s*:\s*(\d+)px/g)) expect(Number(m[1]), m[0]).toBeLessThanOrEqual(720);
    expect(css).not.toMatch(/grid-template-columns:[^;]*\b(?:7[2-9]\d|[89]\d\d|\d{4,})px/);
  });
  it("keeps the nav pinned while the body scrolls, so it stays reachable at 720 by 480", () => {
    expect(rule(".settings-shell__nav")).toMatch(/position:\s*sticky/);
    expect(rule(".settings-shell__nav")).toMatch(/inset-block-start:\s*0/);
  });
  it("moves the nav above the body with a container query, not a viewport breakpoint", () => {
    expect(css).toMatch(/@container settings \(inline-size < 720px\)/);
    expect(css).not.toMatch(/@media\s*\([^)]*(?:width|height)/);
  });
  it("scrolls a focused control clear of the sticky change strip, even with an error line in it", () => {
    expect(rule(".settings-shell .page__body")).toMatch(/scroll-padding-block-end:\s*calc\(var\(--space-8\)\s*\*\s*5\)/);
  });
  it("names the container its query asks", () => {
    expect(rule(".settings-shell")).toMatch(/container:\s*settings\s*\/\s*inline-size/);
  });
});
