import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { DEFAULT_CHOICE, PROJECTS, REMEMBERED } from "./fixtures";
import { LAUNCH_GROUPS, LAUNCH_PROGRAMS, applyRemembered } from "./launchChoice";
import { Launcher, type LauncherView } from "./Launcher";

const view = (p: Partial<LauncherView> = {}): LauncherView => ({ choice: DEFAULT_CHOICE, projects: PROJECTS, query: null, activeIndex: 0, ...p });
const html = (v: LauncherView = view()) => renderToStaticMarkup(<Launcher view={v} />);
const count = (s: string, re: RegExp) => (s.match(re) ?? []).length;

describe("Launcher: empty", () => {
  const h = html();
  it("is a modal dialog with an accessible sentence", () => {
    expect(h).toContain('role="dialog"');
    expect(h).toContain('aria-modal="true"');
    expect(h).toContain("Start Claude in my-app on main, Manual mode, Balanced, no worktree");
  });
  it("reads as one sentence of chips, in order", () => {
    const order = ["Claude", "my-app · main", "Manual", "Balanced", "worktree"].map((s) => h.indexOf(s));
    expect(order.every((i) => i >= 0)).toBe(true);
    expect([...order].sort((a, b) => a - b)).toEqual(order);
    expect(h).toContain(" in ");
  });
  it("names the program chip: a combobox does not take its name from its text", () => {
    expect(h).toMatch(/role="combobox"[^>]*aria-label="Program: Claude"|aria-label="Program: Claude"[^>]*role="combobox"/);
    expect(html(view({ choice: { ...DEFAULT_CHOICE, program: "powershell" } }))).toContain('aria-label="Program: PowerShell"');
  });
  it("makes every chip a button the keyboard reaches with Tab", () => {
    expect(count(h, /class="launcher__chip"/g)).toBe(5);
    expect(h).not.toContain('tabindex="-1" class="launcher__chip"');
  });
  it("has a prompt line and a hint line naming Enter and Esc", () => {
    expect(h).toContain("<textarea");
    expect(h).toContain("Enter");
    expect(h).toContain("Esc");
  });
  it("shows no project list, no program list and no remembered note", () => {
    expect(h).not.toContain('role="listbox"');
    expect(h).not.toContain("Remembered for");
  });
  it("shows the worktree chip as a checkbox", () => {
    expect(h).toContain('role="checkbox"');
    expect(h).toContain('aria-checked="false"');
  });
  it("is set like the prompt line: a prompt glyph, no card around the field", () => {
    expect(h).toContain('class="launcher__line"');
    expect(h).toContain('class="launcher__glyph"');
    expect(h).toContain('<path d="M3.5 3.5L8.5 8l-5 4.5">');
    expect(h).not.toContain("›");
  });
  it("follows the vendor: the sheet carries the agent so its accent does", () => {
    expect(h).toContain('data-agent="claude"');
    expect(html(view({ choice: { ...DEFAULT_CHOICE, program: "codex" } }))).toContain('data-agent="codex"');
  });
});

describe("Launcher: the first chip offers agents, shells and tools", () => {
  const h = html(view({ programsOpen: true }));
  it("opens a listbox grouped under Agents, Shells and Tools", () => {
    expect(h).toMatch(/id="[^"]*-programs"/);
    expect(h).toContain('aria-expanded="true"');
    for (const g of LAUNCH_GROUPS) expect(h, g).toContain(`>${g}<`);
    const order = LAUNCH_GROUPS.map((g) => h.indexOf(`>${g}<`));
    expect([...order].sort((a, b) => a - b)).toEqual(order);
  });
  it("lists every program as an option with its mark, and says which is the current one", () => {
    expect(count(h, /role="option"/g)).toBe(LAUNCH_PROGRAMS.length);
    for (const p of LAUNCH_PROGRAMS) expect(h, p.id).toContain(p.label);
    expect(h).toMatch(/role="option"[^>]*aria-selected="true"[^>]*id="[^"]*-prog-0"|id="[^"]*-prog-0"[^>]*aria-selected="true"/);
    expect(h).toMatch(/aria-activedescendant="[^"]*-prog-0"/);
  });
  it("closed, it points at nothing, so no dangling reference", () => {
    const closed = html();
    expect(closed).not.toMatch(/-programs/);
    expect(closed).not.toContain("aria-activedescendant");
  });
  it("keeps the folder list closed while it is open", () => {
    expect(h).not.toContain('aria-label="Folders"');
  });
});

describe("Launcher: a manual CLI is a plain terminal", () => {
  const manual = view({ choice: { ...DEFAULT_CHOICE, program: "powershell" } });
  const h = html(manual);
  it("has only the program and the folder chips: no mode, pace or worktree", () => {
    expect(count(h, /class="launcher__chip"/g)).toBe(2);
    expect(h).not.toContain("Balanced");
    expect(h).not.toContain("worktree");
    expect(h).not.toContain('role="checkbox"');
  });
  it("says what it starts, in a shorter sentence", () => {
    expect(h).toContain("Start PowerShell in my-app on main");
    expect(h).not.toContain("mode,");
  });
  it("asks for a first command, not a first prompt, and carries no vendor accent", () => {
    expect(h).toContain("Run a command first (optional)");
    expect(h).toContain('aria-label="First command (optional)"');
    expect(h).not.toMatch(/class="launcher__sheet"[^>]*data-agent=/);
    expect(h).toContain('data-manual="true"');
  });
  it("does not show remembered agent choices", () => {
    expect(html(view({ ...manual, remembered: REMEMBERED["my-app"] }))).not.toContain("Remembered for");
  });
  it("an ssh session asks where to connect", () => {
    expect(html(view({ choice: { ...DEFAULT_CHOICE, program: "ssh" } }))).toContain("Where to connect, e.g. user@host");
  });
});

describe("Launcher: filtering", () => {
  const h = html(view({ query: "ap" }));
  it("turns the project chip into a combobox with a listbox of matches", () => {
    expect(h).toContain('role="combobox"');
    expect(h).toContain('role="listbox"');
    expect(h).toContain('aria-expanded="true"');
  });
  it("highlights what matched, and points at the active option", () => {
    expect(h).toContain("<mark");
    const target = /aria-activedescendant="([^"]*-opt-0)"/.exec(h)?.[1];
    expect(target).toBeTruthy();
    expect(h).toContain(`id="${target}"`);
  });
  it("shows an empty state when nothing matches", () => {
    expect(html(view({ query: "zzz" }))).toContain("No folder matches");
  });
  it("keeps the 'no match' message out of the listbox, where only options may live", () => {
    const empty = html(view({ query: "zzz" }));
    expect(empty).not.toContain('role="listbox"');
    expect(empty).toMatch(/<p class="launcher__empty" role="status">No folder matches/);
    expect(empty).not.toMatch(/aria-controls=/);
    expect(empty).not.toContain("aria-activedescendant");
  });
  it("renders hostile text as text", () => {
    const evil = [{ ...PROJECTS[0]!, name: "<img src=x onerror=alert(1)>" }];
    const out = html(view({ query: "i", projects: evil, choice: { ...DEFAULT_CHOICE, projectId: PROJECTS[0]!.id } }));
    expect(out).not.toContain("<img");
  });
});

describe("Launcher: remembered for this project", () => {
  const choice = applyRemembered(DEFAULT_CHOICE, REMEMBERED["my-app"]);
  const h = html(view({ choice, remembered: REMEMBERED["my-app"] }));
  it("applies what was remembered and says so", () => {
    expect(h).toContain("Plan");
    expect(h).toContain("Quick");
    expect(h).toContain("Remembered for my-app: Plan · Quick. Change a choice to update it.");
  });
});

describe("Launcher: the number the new session takes", () => {
  it("says which session number it becomes, so you know what to call it", () => {
    const h = html(view({ nextNumber: 4 }));
    expect(h).toContain("Session 4");
    expect(h).toContain("launcher__next");
  });
  it("says nothing when the parent does not know it", () => {
    expect(html()).not.toContain("launcher__next");
  });
});

describe("Launcher: hygiene", () => {
  it("never puts a literal colour in markup", () => {
    expect(html()).not.toMatch(/#[0-9a-f]{3,8}\b|rgb\(|oklch\(/i);
    expect(html(view({ programsOpen: true }))).not.toMatch(/#[0-9a-f]{3,8}\b|rgb\(|oklch\(/i);
  });
  it("uses sentence case, with no exclamation marks", () => {
    expect(html()).not.toContain("!");
  });
});

describe("Launcher: ids and chip names", () => {
  it("uses ids that are unique when several launchers are on one page (the review page shows many)", () => {
    const h = renderToStaticMarkup(
      <>
        <Launcher view={view({ programsOpen: true })} />
        <Launcher view={view({ programsOpen: true })} />
        <Launcher view={view({ query: "ap" })} />
        <Launcher view={view({ query: "ap" })} />
      </>,
    );
    const ids = [...h.matchAll(/ id="([^"]+)"/g)].map((m) => m[1]);
    expect(new Set(ids).size).toBe(ids.length);
  });
  it("names the mode and pace chips so a screen reader knows what each one is, with the visible word inside the name", () => {
    const h = html();
    expect(h).toContain('aria-label="Mode: Manual"');
    expect(h).toContain('aria-label="Pace: Balanced"');
  });
  it("points its description at an element that exists", () => {
    const h = html();
    const ref = /aria-describedby="([^"]+)"/.exec(h)?.[1];
    expect(ref).toBeTruthy();
    expect(h).toContain(`id="${ref}"`);
  });
});
