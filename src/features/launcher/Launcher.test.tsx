import type { ComponentProps } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { DEFAULT_CHOICE, PROJECTS, REMEMBERED } from "./fixtures";
import { LAUNCH_AGENTS, LAUNCH_SHELLS, MODES, applyRemembered, type LaunchProgram, type LaunchShell, type LauncherChoice } from "./launchChoice";
import type { ShellOffer } from "./shellAgents";
import { Launcher, type LauncherView } from "./Launcher";
import { confirmScripts } from "../scripts/confirmScripts";
import { scriptsFixture } from "../scripts/fixtures";

const view = (p: Partial<LauncherView> = {}): LauncherView => ({ choice: DEFAULT_CHOICE, projects: PROJECTS, query: null, activeIndex: 0, ...p });
const html = (v: LauncherView = view()) => renderToStaticMarkup(<Launcher view={v} />);
const count = (s: string, re: RegExp) => (s.match(re) ?? []).length;

it("disables Administrator with the selected agent's reason, including a palette-requested start", () => {
  const reason = "Codex 0.162.0 can't run as administrator. Marshell needs Codex 0.163.0 or newer to keep an administrator Codex apart from your other Codex sessions. Update Codex, then try again.";
  const programs = LAUNCH_AGENTS.map((p) => p.id === "codex" ? { ...p, adminUnavailable: reason } : p);
  for (const elevated of [false, true]) {
    const h = renderToStaticMarkup(<Launcher agents={programs} view={view({ choice: { ...DEFAULT_CHOICE, agent: "codex" }, elevated })} />);
    expect(h).toMatch(/aria-checked="false" aria-disabled="true" aria-describedby="([^"]+)-admin-off"/);
    expect(h).toContain(reason.replaceAll("'", "&#x27;"));
    expect(h).not.toContain('data-kind="elevated"');
    expect(h).not.toContain("as administrator</p>");
  }
  const other = renderToStaticMarkup(<Launcher agents={programs} view={view({ elevated: true })} />);
  expect(other).toContain('aria-checked="true" aria-keyshortcuts="Alt+A"');
  expect(other).not.toContain(reason.replaceAll("'", "&#x27;"));
});

describe("Launcher: empty", () => {
  const h = html();
  it("is a modal dialog with an accessible sentence", () => {
    expect(h).toContain('role="dialog"');
    expect(h).toContain('aria-modal="true"');
    expect(h).toContain("Start PowerShell running Claude in my-app on main, Manual mode, Balanced, no worktree");
  });
  it("reads as one sentence of chips, in order", () => {
    const order = ["PowerShell", "Claude", "my-app · main", "Manual", "Balanced", "worktree"].map((s) => h.indexOf(s));
    expect(order.every((i) => i >= 0)).toBe(true);
    expect([...order].sort((a, b) => a - b)).toEqual(order);
    expect(h).toContain(" in ");
  });
  it("the_sentence_starts_with_the_shell_then_the_agent", () => {
    const boxes = [...h.matchAll(/<button[^>]*role="combobox"[^>]*aria-label="([^"]*)"/g)].map((m) => m[1]);
    expect(boxes).toEqual(["Shell: PowerShell", "Agent: Claude"]);
    expect(h.indexOf("Shell: PowerShell")).toBeLessThan(h.indexOf("Agent: Claude"));
    expect(h).toContain("Start PowerShell running Claude in my-app on main");
    expect(h).toContain("<span> running </span>");
  });
  it("names both chips: a combobox does not take its name from its text", () => {
    expect(html(view({ choice: { ...DEFAULT_CHOICE, shell: "cmd" } }))).toContain('aria-label="Shell: Command Prompt"');
    expect(html(view({ choice: { ...DEFAULT_CHOICE, agent: null } }))).toContain('aria-label="Agent: none"');
  });
  it("makes every chip a button the keyboard reaches with Tab", () => {
    expect(count(h, /class="launcher__chip"/g)).toBe(7);
    expect(h).not.toContain('tabindex="-1" class="launcher__chip"');
  });
  it("has a prompt line and a hint line naming Enter and Esc", () => {
    expect(h).toContain("<textarea");
    expect(h).toContain("Enter");
    expect(h).toContain("Esc");
  });
  it("shows no project list, no shell or agent list and no remembered note", () => {
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
    expect(html(view({ choice: { ...DEFAULT_CHOICE, agent: "codex" } }))).toContain('data-agent="codex"');
  });
});

describe("Launcher: the shell chip offers the shells", () => {
  const h = html(view({ shellsOpen: true }));
  it("opens a listbox of the shells, with their marks, and says which is the current one", () => {
    expect(h).toMatch(/id="[^"]*-shells"/);
    expect(h).toContain('aria-expanded="true"');
    expect(h).toContain('aria-label="Shells"');
    expect(count(h, /role="option"/g)).toBe(LAUNCH_SHELLS.length);
    for (const s of LAUNCH_SHELLS) expect(h, s.id).toContain(s.label);
    expect(h).toMatch(/aria-activedescendant="[^"]*-shell-0"/);
    expect(h).toMatch(/data-current="true"/);
  });
  it("has no headings and no Tools", () => {
    expect(h).not.toContain(">Tools<");
    expect(h).not.toContain(">Agents<");
  });
  it("closed, it points at nothing, so no dangling reference", () => {
    const closed = html();
    expect(closed).not.toMatch(/-shells|-agents/);
    expect(closed).not.toContain("aria-activedescendant");
  });
  it("keeps the folder list closed while it is open", () => {
    expect(h).not.toContain('aria-label="Folders"');
  });
});

describe("Launcher: the agent chip offers Just the shell, then the agents", () => {
  const h = html(view({ agentsOpen: true }));
  it("the_agent_list_starts_with_just_the_shell", () => {
    expect(h).toContain('aria-label="Agents in PowerShell"');
    expect(count(h, /role="option"/g)).toBe(LAUNCH_AGENTS.length + 1);
    const options = [...h.matchAll(/role="option"[^>]*>(.*?)<\/div>/g)].map((m) => m[1]!.replace(/<[^>]*>/g, ""));
    expect(options[0]).toBe("Just the shell");
    expect(options.slice(1)).toEqual(LAUNCH_AGENTS.map((p) => p.label));
    expect(h).toMatch(/aria-activedescendant="[^"]*-agent-0"/);
  });
});

describe("Launcher: what the shell offers", () => {
  const ready: ShellOffer = {
    state: "ready",
    offered: LAUNCH_AGENTS.slice(0, 2),
    missing: [
      { label: "Antigravity", problem: "not_installed" },
      { label: "GitHub Copilot", problem: "not_driven_here" },
    ],
  };
  const ubuntu: LaunchShell[] = [{ id: "ubuntu", label: "Ubuntu", kind: "wsl", family: "wsl" }];
  const on = (v: Partial<LauncherView>, p: Partial<ComponentProps<typeof Launcher>> = {}) =>
    renderToStaticMarkup(<Launcher shells={ubuntu} view={view({ choice: { ...DEFAULT_CHOICE, shell: "ubuntu" }, ...v })} {...p} />);
  it("lists only the agents the shell offers", () => {
    const h = on({ agentsOpen: true }, { offer: ready });
    expect(count(h, /role="option"/g)).toBe(3);
    expect(h).not.toContain(">Cursor<");
  });
  it("the_missing_line_sits_under_the_agent_list_outside_the_listbox", () => {
    const h = on({ agentsOpen: true }, { offer: ready });
    const line = "Not in Ubuntu: Antigravity (not installed), GitHub Copilot (Marshell runs it in WSL later)";
    expect(h).toContain(line);
    const list = /<div[^>]*role="listbox"[^>]*>(.*)/.exec(h)![1]!;
    const listEnd = h.indexOf("</div></div></div>", h.indexOf('role="listbox"'));
    expect(h.indexOf(line)).toBeGreaterThan(listEnd);
    expect(list.slice(0, list.indexOf(line))).toContain("</div></div></div>");
  });
  it("checking_disables_start_and_describes_the_agent_chip", () => {
    const h = on({}, { offer: { state: "checking" } });
    expect(h).toMatch(/class="launcher__start"[^>]*disabled=""/);
    const ref = /aria-label="Agent: Claude"[^>]*aria-describedby="([^"]+)"/.exec(h)?.[1];
    expect(ref).toMatch(/-checking$/);
    expect(h).toMatch(new RegExp(`<p class="launcher__reason" id="${ref}">Checking which agents Ubuntu has…</p>`));
    expect(on({}, { offer: ready })).not.toContain("Checking which agents");
    expect(on({}, { offer: ready })).not.toMatch(/class="launcher__start"[^>]*disabled=""/);
  });
  it("an_unreached_shell_shows_its_note_and_start_stays_enabled", () => {
    const h = on({}, { offer: { state: "unreached" } });
    expect(h).toContain("Marshell couldn&#x27;t reach Ubuntu, so only the shell is offered.");
    expect(h).not.toMatch(/class="launcher__start"[^>]*disabled=""/);
    // The agent that was chosen is not started: the sentence and chip say so.
    expect(h).toContain('aria-label="Agent: none"');
    expect(h).toContain("Start Ubuntu with no agent in");
    expect(on({ agentsOpen: true }, { offer: { state: "unreached" } })).toContain('aria-label="Agents in Ubuntu"');
  });
  it("drops an agent the shell lacks and shows Just the shell", () => {
    const h = on({ choice: { ...DEFAULT_CHOICE, shell: "ubuntu", agent: "cursor" } }, { offer: ready });
    expect(h).toContain('aria-label="Agent: none"');
    expect(h).not.toContain("Mode:");
  });
  it("command_prompt_replaces_the_prompt_line_with_its_note", () => {
    const cmd: LaunchShell[] = [{ id: "cmd", label: "Command Prompt", kind: "cmd", family: "cmd" }];
    const h = renderToStaticMarkup(<Launcher shells={cmd} view={view({ choice: { ...DEFAULT_CHOICE, shell: "cmd" } })} />);
    expect(h).toContain("Command Prompt can&#x27;t pass a first prompt. Type it once Claude starts.");
    expect(h).not.toContain("<textarea");
    expect(html()).toContain("<textarea");
  });
  it("with_no_agent_the_sentence_reads_with_no_agent", () => {
    const h = html(view({ choice: { ...DEFAULT_CHOICE, agent: null } }));
    expect(h).toContain("<span> with </span>");
    expect(h).toContain("no agent");
    expect(h).toContain("Start PowerShell with no agent in my-app on main");
    expect(h).not.toContain("<textarea");
  });
});

describe("Launcher: just the shell is a plain terminal", () => {
  const manual = view({ choice: { ...DEFAULT_CHOICE, agent: null } });
  const h = html(manual);
  it("has the shell, agent, folder and worktree chips: no mode or pace", () => {
    expect(count(h, /class="launcher__chip"/g)).toBe(5);
    expect(h).not.toContain("Balanced");
    expect(h).toContain('role="checkbox"');
  });
  it("says what it starts, in a shorter sentence", () => {
    expect(h).toContain("Start PowerShell with no agent in my-app on main");
    expect(h).not.toContain("mode,");
  });
  it("has no prompt line and carries no vendor accent", () => {
    expect(h).not.toContain("<textarea");
    expect(h).not.toMatch(/class="launcher__sheet"[^>]*data-agent=/);
    expect(h).toContain('data-manual="true"');
  });
  it("does not show remembered agent choices", () => {
    expect(html(view({ ...manual, remembered: REMEMBERED["my-app"] }))).not.toContain("Remembered for");
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
  it("keeps the 'no match' message out of the listbox, where only options may live; the listbox holds just Browse…", () => {
    const empty = html(view({ query: "zzz" }));
    expect(empty).toMatch(/<p class="launcher__empty" role="status">No folder matches/);
    const list = /<ul[^>]*role="listbox"[^>]*>(.*?)<\/ul>/.exec(empty)?.[1] ?? "";
    expect(count(list, /role="option"/g)).toBe(1);
    expect(list).toContain("Browse…");
    expect(list).not.toContain("No folder matches");
  });
  it("renders hostile text as text", () => {
    const evil = [{ ...PROJECTS[0]!, name: "<img src=x onerror=alert(1)>" }];
    const out = html(view({ query: "i", projects: evil, choice: { ...DEFAULT_CHOICE, projectId: PROJECTS[0]!.id } }));
    expect(out).not.toContain("<img");
  });
});

describe("Launcher: the path row and Browse…", () => {
  const options = (out: string) => [...out.matchAll(/<li[^>]*role="option"[^>]*>.*?<\/li>/g)].map((m) => m[0]);
  const text = (li: string) => li.replace(/<[^>]*>/g, "");
  const found = { path: "C:\\dev", state: "found" as const, folder: { path: "C:\\dev", name: "dev", branch: null } };
  it("ends the list with Browse… and its Ctrl+O hint", () => {
    const o = options(html(view({ query: "" })));
    expect(o.length).toBe(PROJECTS.length + 1);
    expect(text(o.at(-1)!)).toContain("Browse…");
    expect(o.at(-1)).toContain("<kbd>Ctrl+O</kbd>");
  });
  it("puts Use <path> first for a folder that exists, and it can be taken", () => {
    const o = options(html(view({ query: "C:\\dev", pathCheck: found })));
    expect(text(o[0]!)).toBe("Use C:\\dev");
    expect(o[0]).toContain('<span class="launcher__opt-name" dir="auto">Use</span>');
    expect(o[0]).toContain("launcher__opt-path");
    expect(o[0]).not.toContain("aria-disabled");
  });
  it("shows No folder at <path> as disabled for a missing one", () => {
    const o = options(html(view({ query: "C:\\nope", pathCheck: { path: "C:\\nope", state: "missing" } })));
    expect(text(o[0]!)).toBe("No folder at C:\\nope");
    expect(o[0]).toContain('aria-disabled="true"');
  });
  it("shows Couldn't check <path> as disabled when the check itself failed", () => {
    const o = options(html(view({ query: "C:\\dev", pathCheck: { path: "C:\\dev", state: "failed" } })));
    expect(text(o[0]!)).toBe("Couldn&#x27;t check C:\\dev"); // the markup escapes the apostrophe
    expect(o[0]).toContain('aria-disabled="true"');
  });
  it("shows Checking <path>… as disabled while the core answers", () => {
    const o = options(html(view({ query: "C:\\de", pathCheck: { path: "C:\\de", state: "checking" } })));
    expect(text(o[0]!)).toBe("Checking C:\\de…");
    expect(o[0]).toContain('aria-disabled="true"');
  });
  it("counts the path row in the active descendant and keeps the list open", () => {
    const out = html(view({ query: "C:\\dev", pathCheck: found, activeIndex: 0 }));
    expect(out).toContain('aria-expanded="true"');
    expect(out).toMatch(/aria-activedescendant="[^"]*-opt-0"/);
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
  it("shows the number while the start is pending", () => {
    expect(renderToStaticMarkup(<Launcher view={view({ nextNumber: 2 })} pending />)).toContain("Session 2");
  });
  it("says nothing when the parent does not know it", () => {
    expect(html()).not.toContain("launcher__next");
  });
});

describe("Launcher: chips", () => {
  it("shows the mode, pace and worktree chips by default", () => {
    const h = html();
    expect(h).toContain("Mode: Manual");
    expect(h).toContain('role="checkbox"');
  });
  it("keeps the mode chip and drops the pace chip when paceChip is false (the worktree chip is covered below)", () => {
    const h = renderToStaticMarkup(<Launcher view={view({ choice: { ...DEFAULT_CHOICE, projectId: "scratch" } })} paceChip={false} />);
    expect(count(h, /class="launcher__chip"/g)).toBe(5);
    expect(h).toContain("Mode: Manual");
    expect(h).not.toContain("Pace:");
    expect(h).toContain("Start PowerShell running Claude in scratch");
  });
  it("shows a caution mark in the chip for a mode that runs without asking, and not for the others", () => {
    const chip = (mode: LauncherChoice["mode"]) => html(view({ choice: { ...DEFAULT_CHOICE, mode } }));
    expect(chip("bypass")).toContain("Mode: Bypass");
    expect(chip("bypass")).toContain("⚠");
    expect(chip("full-auto")).toContain("⚠");
    for (const m of ["manual", "plan", "auto-edit"] as const) expect(chip(m), m).not.toContain("⚠");
  });
  it("shows each of the five modes by its label", () => {
    const labels = ["Manual", "Plan", "Auto-edit", "Full auto", "Bypass"];
    MODES.forEach((m, i) => expect(html(view({ choice: { ...DEFAULT_CHOICE, mode: m } }))).toContain(`Mode: ${labels[i]}`));
  });
});

describe("Launcher: the worktree chip in the live launcher", () => {
  const live = (v: LauncherView = view(), paceChip = false) => renderToStaticMarkup(<Launcher view={v} paceChip={paceChip} />);
  const inScratch = { ...DEFAULT_CHOICE, projectId: "scratch" };
  it("is shown when the folder is in git, with the mode chip but no pace chip", () => {
    const h = live();
    expect(h).toContain('role="checkbox"');
    expect(h).toContain('aria-checked="false"');
    expect(count(h, /class="launcher__chip"/g)).toBe(6);
    expect(h).toContain("Mode: Manual");
    expect(h).not.toContain("Pace:");
  });
  it("is hidden for a folder with no branch", () => {
    const h = live(view({ choice: inScratch }));
    expect(h).not.toContain(">worktree<");
    expect(count(h, /class="launcher__chip"/g)).toBe(5);
  });
  it("is offered for a shell too: a worktree works for any program", () => {
    expect(live(view({ choice: { ...DEFAULT_CHOICE, agent: null } }))).toContain("worktree<");
    expect(live(view({ choice: { ...DEFAULT_CHOICE, agent: null, projectId: "scratch" } }))).not.toContain('worktree<');
  });
});

describe("Launcher: the task name", () => {
  const on = { ...DEFAULT_CHOICE, worktree: true };
  const h = (v: Partial<LauncherView> = {}, paceChip = false) => renderToStaticMarkup(<Launcher view={view({ choice: on, taskName: "fix-login", ...v })} paceChip={paceChip} />);
  it("shows a labelled text field with the name once the chip is on", () => {
    const out = h();
    const forId = /<label[^>]*for="([^"]+)"[^>]*>Task name<\/label>/.exec(out)?.[1];
    expect(forId).toBeTruthy();
    expect(out).toMatch(new RegExp(`<input[^>]*id="${forId}"[^>]*value="fix-login"|<input[^>]*value="fix-login"[^>]*id="${forId}"`));
  });
  it("is not there while the chip is off or in a folder with no branch", () => {
    expect(h({ choice: DEFAULT_CHOICE })).not.toContain("Task name");
    expect(h({ choice: { ...on, projectId: "scratch" } })).not.toContain("Task name");
  });
  it("is there for just the shell, which has no prompt to name it from: an empty name hints at task-…", () => {
    const out = h({ choice: { ...on, agent: null }, taskName: "" });
    expect(out).toContain("Task name");
    expect(out).toContain("Branch marshell/task-…");
  });
  it("hints at the branch below the field, and at task-… when the name is empty", () => {
    const out = h();
    expect(out).toContain("Branch marshell/fix-login");
    expect(out.indexOf("Task name")).toBeLessThan(out.indexOf("Branch marshell/fix-login"));
    expect(h({ taskName: "" })).toContain("Branch marshell/task-…");
  });
  it("hints at the name the core will make, not the raw text", () => {
    expect(h({ taskName: "Fix Login!" })).toContain("Branch marshell/fix-login");
  });
  it("the hint is tied to the field for a screen reader", () => {
    const out = h();
    const hint = /aria-describedby="([^"]+)"/.exec(out.slice(out.indexOf("Task name")))?.[1];
    expect(hint).toBeTruthy();
    expect(out).toContain(`id="${hint}"`);
  });
});

describe("Launcher: confirm a task", () => {
  const on = { ...DEFAULT_CHOICE, worktree: true };
  const cmds = ["git worktree add -b marshell/x C:/w/x main", "git status"];
  const h = (confirm: LauncherView["confirm"], extra: Partial<ComponentProps<typeof Launcher>> = {}) =>
    renderToStaticMarkup(<Launcher view={view({ choice: on, taskName: "x", confirm })} paceChip={false} {...extra} />);
  it("swaps the sentence for a titled view with the commands and two buttons", () => {
    const out = h({ state: "ready", commands: cmds });
    expect(out).toContain("Start a task in a new worktree");
    expect(out).toContain('aria-label="Commands Marshell will run"');
    expect(count(out, /<code[ >]/g)).toBe(2);
    expect(out).toContain(">Create and start<");
    expect(out).toContain(">Back<");
    expect(out.indexOf("Create and start")).toBeLessThan(out.indexOf(">Back<"));
    expect(out).not.toContain("launcher__sentence");
    expect(out).not.toContain("<textarea");
    expect(out).not.toContain("Task name");
  });
  it("names the dialog for the task, and keeps it modal", () => {
    const out = h({ state: "ready", commands: cmds });
    expect(out).toContain('role="dialog"');
    expect(out).toContain('aria-label="Start a task in a new worktree"');
  });
  const unavailable = (label: string) => new RegExp(`<button[^>]*aria-disabled="true"[^>]*>${label}</button>`);
  it("shows the plan's refusal in place of the commands, with Create and start unavailable", () => {
    const out = h({ state: "refused", reason: "That folder is not a git repository." });
    expect(out).toMatch(/<p class="launcher__error" role="alert">That folder is not a git repository\.<\/p>/);
    expect(out).not.toContain("<ol");
    expect(out).toMatch(unavailable("Create and start"));
    expect(out).not.toMatch(unavailable("Back"));
  });
  it("waits for the plan with plain text, not a live message, and Create and start unavailable", () => {
    const out = h({ state: "loading" });
    expect(out).toMatch(unavailable("Create and start"));
    expect(out).toMatch(/<p class="launcher__pending">Checking the folder…<\/p>/);
    expect(out).not.toContain("<ol");
  });
  it("never uses the disabled attribute in the confirm view: a disabled button drops focus", () => {
    for (const out of [h({ state: "loading" }), h({ state: "refused", reason: "x" }), h({ state: "ready", commands: cmds }, { busy: true })]) expect(out).not.toMatch(/<button[^>]*\sdisabled=/);
  });
  it("says Creating the worktree… while the create is pending, and makes both buttons unavailable", () => {
    const out = h({ state: "ready", commands: cmds }, { busy: true });
    expect(out).toMatch(/<p[^>]*role="status"[^>]*>Creating the worktree…<\/p>/);
    expect(out).toMatch(unavailable("Create and start"));
    expect(out).toMatch(unavailable("Back"));
  });
  it("shows the parent's text in the one status line once the worktree exists and the session is starting", () => {
    const out = h({ state: "ready", commands: cmds }, { busy: true, busyText: "Starting Claude…" });
    expect(count(out, /role="status"/g)).toBe(1);
    expect(out).toMatch(/<p[^>]*role="status"[^>]*>Starting Claude…<\/p>/);
    expect(out).not.toContain("Creating the worktree…");
  });
  it("leaves both buttons available when the plan is ready", () => {
    expect(h({ state: "ready", commands: cmds })).not.toContain("aria-disabled");
  });
  it("shows a create failure once, as an alert, under the buttons", () => {
    const out = h({ state: "ready", commands: cmds }, { error: "Git said: fatal: a branch named 'marshell/x' already exists" });
    expect(count(out, /role="alert"/g)).toBe(1);
    expect(out.indexOf(">Back<")).toBeLessThan(out.indexOf('role="alert"'));
  });
  it("says its keys: Enter creates, Esc goes back", () => {
    const out = h({ state: "ready", commands: cmds });
    expect(out).toMatch(/<kbd>Enter<\/kbd>/);
    expect(out).toMatch(/<kbd>Esc<\/kbd>/);
  });
});

describe("Launcher: confirm a task with a marshell.json", () => {
  const on = { ...DEFAULT_CHOICE, worktree: true };
  const cmds = ["git worktree add -b marshell/x C:/w/x main", "git status"];
  const h = (scripts: ReturnType<typeof confirmScripts>, extra: Partial<ComponentProps<typeof Launcher>> = {}) =>
    renderToStaticMarkup(<Launcher view={view({ choice: on, taskName: "x", confirm: { state: "ready", commands: cmds, ...(scripts === undefined ? {} : { scripts }) } })} paceChip={false} {...extra} />);
  it("asks for an untrusted file above the git commands, with Allow and create and Create without scripts", () => {
    const out = h(confirmScripts(scriptsFixture()));
    expect(out).toContain("Allow this repo&#x27;s scripts?");
    expect(out).toContain(">Allow and create<");
    expect(out).toContain(">Create without scripts<");
    expect(out.indexOf("Allow this repo")).toBeLessThan(out.indexOf("git worktree add"));
    expect(out).toContain(">Back<");
  });
  it("leaves out Create and start while the question is open, so only a deliberate press allows", () => {
    expect(h(confirmScripts(scriptsFixture()))).not.toContain(">Create and start<");
  });
  it("adds setup: <script> as the last command line for a trusted setup", () => {
    const out = h(confirmScripts(scriptsFixture({ trusted: true })));
    const codes = [...out.matchAll(/<code[^>]*>([^<]*)<\/code>/g)].map((m) => m[1]);
    expect(codes).toEqual([...cmds, "setup: pnpm install"]);
    expect(out).toContain(">Create and start<");
    expect(out).not.toContain("Allow this repo");
  });
  it("shows a problem as one line and keeps Create and start", () => {
    const out = h(confirmScripts(scriptsFixture({ problem: "marshell.json: bad json" })));
    expect(out).toContain("marshell.json: bad json");
    expect(out).not.toContain("Allow this repo");
    expect(out).toContain(">Create and start<");
    expect(count(out, /<code[ >]/g)).toBe(2);
  });
  it("shows only the card when the task already exists, with no commands and no Back", () => {
    const out = h({ kind: "ask", scripts: scriptsFixture(), allowLabel: "Allow and run setup", cancelLabel: "Start without setup", only: true });
    expect(out).toContain(">Allow and run setup<");
    expect(out).toContain(">Start without setup<");
    expect(out).not.toContain("git worktree add");
    expect(out).not.toContain(">Back<");
    expect(out).not.toContain("Marshell will run these commands");
  });
});

describe("Launcher: errors and busy", () => {
  const withError = renderToStaticMarkup(<Launcher view={view()} error="Command Prompt can't pass a prompt safely." />);
  it("launcher shows the error and stays open", () => {
    expect(withError).toMatch(/<p class="launcher__error" role="alert">Command Prompt can&#x27;t pass a prompt safely\.<\/p>/);
    expect(withError).toContain('role="dialog"');
    expect(withError.indexOf("launcher__start")).toBeLessThan(withError.indexOf("launcher__error"));
  });
  it("shows no alert when there is no error", () => {
    expect(html()).not.toContain('role="alert"');
  });
  it("disables Start while busy and keeps its label", () => {
    const h = renderToStaticMarkup(<Launcher view={view()} busy />);
    expect(h).toMatch(/<button type="button" class="launcher__start" disabled="">Start <kbd>Enter<\/kbd><\/button>/);
    expect(html()).not.toContain("disabled");
  });
});

describe("Launcher: live programs", () => {
  const programs: LaunchProgram[] = [LAUNCH_AGENTS[0]!];
  const shells: LaunchShell[] = [
    { id: "cmd", label: "Command Prompt", kind: "cmd", family: "cmd" },
    { id: "pwsh", label: "PowerShell 7", kind: "powershell", family: "power_shell" },
  ];
  const live = (choice: LauncherChoice, open = false) => renderToStaticMarkup(<Launcher view={view({ choice, agentsOpen: open })} agents={programs} shells={shells} paceChip={false} />);
  it("names a profile shell with its label and treats no agent as a plain terminal", () => {
    const h = live({ ...DEFAULT_CHOICE, shell: "cmd", agent: null });
    expect(h).toContain('aria-label="Shell: Command Prompt"');
    expect(h).toContain("Start Command Prompt with no agent in my-app on main");
    expect(h).toContain('data-manual="true"');
  });
  it("hides the first-prompt box for just the shell, since only agents take a prompt", () => {
    expect(live({ ...DEFAULT_CHOICE, shell: "pwsh", agent: null })).not.toContain("<textarea");
    expect(live({ ...DEFAULT_CHOICE, shell: "pwsh" })).toContain("<textarea");
  });
  describe("an agent whose manifest maps some modes", () => {
    const agentOf = (id: string, modes: LaunchProgram["modes"], extra: Partial<LaunchProgram> = {}): LaunchProgram => ({
      id,
      kind: id as LaunchProgram["kind"],
      label: id,
      cli: id,
      group: "Agents",
      first: `What should ${id} do first? (optional)`,
      ...(modes === undefined ? {} : { modes }),
      ...extra,
    });
    const list = [agentOf("cursor", ["manual", "plan", "bypass"]), agentOf("opencode", []), agentOf("kimi", ["manual", "bypass"], { noFirst: true })];
    const on = (program: string, mode: LauncherChoice["mode"] = "manual") => renderToStaticMarkup(<Launcher view={view({ choice: { ...DEFAULT_CHOICE, agent: program, mode } })} agents={list} paceChip={false} />);
    it("shows the mode chip for an agent with modes", () => {
      expect(on("cursor", "plan")).toContain("Mode: Plan");
    });
    it("shows no mode chip, and no mode in the sentence, for an agent with none", () => {
      const h = on("opencode");
      expect(h).not.toContain("Mode:");
      expect(h).not.toContain("mode,");
      expect(h).toContain("Start PowerShell running opencode in my-app on main");
    });
    it("hides the first-prompt box for a CLI that takes no first prompt and keeps its mode chip", () => {
      const h = on("kimi");
      expect(h).not.toContain("<textarea");
      expect(h).toContain("Mode: Manual");
    });
  });
  it("lists only the agents it was given, after Just the shell", () => {
    const h = live({ ...DEFAULT_CHOICE, shell: "pwsh" }, true);
    expect(count(h, /role="option"/g)).toBe(2);
    expect(h).not.toContain(">Tools<");
  });
});

describe("Launcher: hygiene", () => {
  it("never puts a literal colour in markup", () => {
    expect(html()).not.toMatch(/#[0-9a-f]{3,8}\b|rgb\(|oklch\(/i);
    expect(html(view({ shellsOpen: true }))).not.toMatch(/#[0-9a-f]{3,8}\b|rgb\(|oklch\(/i);
  });
  it("uses sentence case, with no exclamation marks", () => {
    expect(html()).not.toContain("!");
  });
});

describe("Launcher: ids and chip names", () => {
  it("uses ids that are unique when several launchers are on one page (the review page shows many)", () => {
    const h = renderToStaticMarkup(
      <>
        <Launcher view={view({ shellsOpen: true })} />
        <Launcher view={view({ shellsOpen: true })} />
        <Launcher view={view({ agentsOpen: true })} />
        <Launcher view={view({ agentsOpen: true })} />
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

describe("Launcher: the Administrator chip", () => {
  const admin = (v: Partial<LauncherView> = {}, p: Partial<ComponentProps<typeof Launcher>> = {}) => renderToStaticMarkup(<Launcher view={view(v)} {...p} />);
  it("the_admin_chip_toggles_with_alt_a_and_sends_elevated: a checkbox after Pace, off by default, with a shield and Alt+A when on", () => {
    const off = admin();
    expect(off).toContain("Administrator");
    expect(off.indexOf("Balanced")).toBeLessThan(off.indexOf("Administrator"));
    expect(off).toMatch(/role="checkbox"[^>]*aria-checked="false"[^>]*aria-keyshortcuts="Alt\+A"|aria-keyshortcuts="Alt\+A"[^>]*role="checkbox"/);
    expect(off).not.toContain('data-kind="elevated"');
    const on = admin({ elevated: true });
    expect(on).toContain('data-kind="elevated"');
    expect(on).toMatch(/aria-checked="true"[^>]*aria-keyshortcuts="Alt\+A"/);
    expect(on).toContain("as administrator");
  });
  it("a disabled chip does not advertise Alt+A, and an enabled one does", () => {
    const reason = "Not here.";
    const programs = LAUNCH_AGENTS.map((p) => p.id === "codex" ? { ...p, adminUnavailable: reason } : p);
    const off = renderToStaticMarkup(<Launcher agents={programs} view={view({ choice: { ...DEFAULT_CHOICE, agent: "codex" } })} />);
    expect(off).toContain('aria-disabled="true"');
    expect(off).not.toContain("aria-keyshortcuts");
    expect(admin()).toContain('aria-keyshortcuts="Alt+A"');
  });
  it("shows for a shell too", () => {
    expect(admin({ choice: { ...DEFAULT_CHOICE, agent: null } })).toContain("Administrator");
  });
  it("the_admin_chip_is_disabled_off_windows_with_the_reason", () => {
    for (const os of ["mac", "linux"] as const) {
      const h = admin({ elevated: true }, { os });
      expect(h).toContain("Administrator: available on Windows only for now.");
      expect(h).toMatch(/role="checkbox"[^>]*aria-disabled="true"/);
      expect(h).toContain('aria-checked="false"');
      expect(h).not.toContain('data-kind="elevated"');
    }
    expect(admin({}, { os: "windows" })).not.toContain("available on Windows only");
  });
  it("an_agent_with_admin_on_is_manual_and_the_mode_chip_is_disabled", () => {
    const h = admin({ elevated: true, choice: { ...DEFAULT_CHOICE, mode: "bypass" } });
    expect(h).toContain('aria-label="Mode: Manual"');
    expect(h).toMatch(/aria-label="Mode: Manual"[^>]*aria-disabled="true"/);
    expect(h).toContain("An agent running as administrator starts in Manual.");
    expect(admin({ choice: { ...DEFAULT_CHOICE, mode: "bypass" } })).not.toContain("starts in Manual");
    // A shell has no mode chip, so no reason line either.
    expect(admin({ elevated: true, choice: { ...DEFAULT_CHOICE, agent: null } })).not.toContain("starts in Manual");
  });
  it("the_new_tab_waits_for_approval: while an elevated start is pending the launcher says so", () => {
    expect(admin({ elevated: true }, { busy: true, pending: true })).toContain("Waiting for administrator approval…");
    expect(admin({}, { busy: true, pending: true })).not.toContain("Waiting for administrator approval");
    // Lists still loading is not a create in flight.
    expect(admin({ elevated: true }, { busy: true })).not.toContain("Waiting for administrator approval");
  });
});


describe("Launcher: an agent's own skip-every-prompt setting", () => {
  const programs = LAUNCH_AGENTS.map((p) => p.id === "antigravity" ? { ...p, skipsAll: { setting: "dangerouslySkipPermissions" } } : p);
  const skipView = (elevated: boolean, bypassAcknowledged = false) => view({ choice: { ...DEFAULT_CHOICE, agent: "antigravity" }, elevated, bypassAcknowledged });
  it("holds an administrator start until the Bypass box is ticked", () => {
    const h = renderToStaticMarkup(<Launcher view={skipView(true)} agents={programs} />);
    expect(h).toContain('aria-label="Mode: Bypass"');
    expect(h).toContain('type="checkbox"');
    expect(h).toContain("I understand this agent runs as administrator and Bypass lets it run admin commands without asking.");
    expect(h).toMatch(/class="launcher__start"[^>]*disabled=""/);
    const ticked = renderToStaticMarkup(<Launcher view={skipView(true, true)} agents={programs} />);
    expect(ticked).not.toMatch(/class="launcher__start"[^>]*disabled=""/);
    expect(ticked).toContain('checked=""');
    expect(h).toContain("dangerouslySkipPermissions");
    expect(h).not.toContain("starts in Manual");
    expect(h).toContain("Bypass mode");
  });
  it("labels an ordinary start Bypass and enables Start without a checkbox", () => {
    const h = renderToStaticMarkup(<Launcher view={skipView(false)} agents={programs} />);
    expect(h).toContain('aria-label="Mode: Bypass"');
    expect(h).not.toContain('type="checkbox"');
    expect(h).not.toMatch(/class="launcher__start"[^>]*disabled=""/);
  });
  it.each(MODES)("explains why an ordinary %s start is Bypass", (mode) => {
    const v = skipView(false);
    const h = renderToStaticMarkup(<Launcher view={{ ...v, choice: { ...v.choice, mode } }} agents={programs} />);
    expect(h).toContain('aria-label="Mode: Bypass"');
    expect(h).toContain("runs as Bypass because its own dangerouslySkipPermissions setting skips every prompt.");
  });
});


it("holds an administrator adapter start until its own settings have been read", () => {
  const programs = LAUNCH_AGENTS.map((p) => p.id === "antigravity" ? { ...p, adminSetupPending: true } : p);
  const v = view({ choice: { ...DEFAULT_CHOICE, agent: "antigravity" }, elevated: true });
  const h = renderToStaticMarkup(<Launcher view={v} agents={programs} />);
  expect(h).toMatch(/class="launcher__start"[^>]*disabled=""/);
  expect(h).toMatch(/class="launcher__reason"[^>]*>Checking Antigravity&#x27;s settings…<\/p>/);
  expect(renderToStaticMarkup(<Launcher view={{ ...v, elevated: false }} agents={programs} />)).not.toMatch(/class="launcher__start"[^>]*disabled=""/);
});

it("lets an administrator start with unreadable Antigravity settings proceed only after the Bypass confirmation", () => {
  const programs = LAUNCH_AGENTS.map((p) => p.id === "antigravity" ? { ...p, adminSetupFailed: true } : p);
  const v = view({ choice: { ...DEFAULT_CHOICE, agent: "antigravity" }, elevated: true });
  const h = renderToStaticMarkup(<Launcher view={v} agents={programs} />);
  expect(h).toContain("Marshell couldn&#x27;t read Antigravity&#x27;s settings, so it may skip every prompt.");
  expect(h).toContain('type="checkbox"');
  expect(h).toMatch(/class="launcher__start"[^>]*disabled=""/);
  expect(renderToStaticMarkup(<Launcher view={{ ...v, bypassAcknowledged: true }} agents={programs} />)).not.toMatch(/class="launcher__start"[^>]*disabled=""/);
  expect(renderToStaticMarkup(<Launcher view={{ ...v, elevated: false }} agents={programs} />)).not.toContain('type="checkbox"');
});
