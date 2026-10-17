import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { commandShortcut } from "../../lib/appShortcuts";
import { PALETTE_SESSIONS } from "../palette/fixtures";
import { LIVE_COMMANDS, LivePalette, escapeCloses } from "./LivePalette";

describe("LivePalette", () => {
  it("lists only commands the window runs", () => {
    const ids = LIVE_COMMANDS.map((c) => c.id);
    for (const id of ["launcher", "close", "reopen", "rename", "move-up", "move-down", "profiles", "appearance", "split", "unsplit", "layout", "resume", "zoom-pane", "nav-next-pane", "nav-none", "focus-mode", "sidebar", "group-by-project", "copy-recording-path"]) expect(ids, id).toContain(id);
    // Every other row would do nothing when chosen. (Remove-pane, Change-layout and Resume have no key shortcut; the window runs them from the palette.)
    for (const id of ids) expect(["profiles", "appearance", "unsplit", "layout", "resume", "group-by-project", "clean-up-task", "copy-recording-path"].includes(id) || id.startsWith("mode-") || commandShortcut(id) !== null, id).toBe(true);
    for (const id of ["mode-manual", "mode-plan", "mode-auto-edit", "mode-full-auto", "mode-bypass"]) expect(ids, id).toContain(id);
    for (const id of ["doctor", "history", "go-to-session"]) expect(ids, id).not.toContain(id);
  });
  it("offers Resume only when the window does not hide it", () => {
    const show = (hidden?: string[]) => renderToStaticMarkup(<LivePalette sessions={PALETTE_SESSIONS} os="windows" hidden={hidden} onRun={() => {}} onClose={() => {}} />);
    expect(show()).toContain("Resume this session");
    expect(show(["resume"])).not.toContain("Resume this session");
  });
  it("offers Review this task's changes unless the window hides it (the active session is not a task)", () => {
    const show = (hidden?: string[]) => renderToStaticMarkup(<LivePalette sessions={PALETTE_SESSIONS} os="windows" hidden={hidden} onRun={() => {}} onClose={() => {}} />);
    expect(show()).toContain("Review this task");
    expect(show(["review-task"])).not.toContain("Review this task");
    expect(LIVE_COMMANDS.map((c) => c.id)).toContain("review-task");
  });
  it("offers Clean up a missing task unless the window hides it (no task's folder is gone)", () => {
    const show = (hidden?: string[]) => renderToStaticMarkup(<LivePalette sessions={PALETTE_SESSIONS} os="windows" hidden={hidden} onRun={() => {}} onClose={() => {}} />);
    expect(show()).toContain("Clean up a missing task…");
    expect(show(["clean-up-task"])).not.toContain("Clean up a missing task");
  });
  it("shows those commands and not the inert ones", () => {
    const h = renderToStaticMarkup(<LivePalette sessions={PALETTE_SESSIONS} os="windows" onRun={() => {}} onClose={() => {}} />);
    expect(h).toContain("Shell profiles");
    expect(h).toContain("Start a session");
    expect(h).not.toContain("Open Doctor");
    expect(h).toContain("Split: add a pane");
  });
  it("shows the label the window gives a command, and the keymap's own when it gives none", () => {
    const show = (labels?: Record<string, string>) => renderToStaticMarkup(<LivePalette sessions={PALETTE_SESSIONS} os="windows" labels={labels} onRun={() => {}} onClose={() => {}} />);
    expect(show()).toContain("Collapse sidebar to the rail");
    const h = show({ sidebar: "Expand the sidebar" });
    expect(h).toContain("Expand the sidebar");
    expect(h).not.toContain("Collapse sidebar to the rail");
  });
  it("names the grouping command for what it does next", () => {
    const show = (labels?: Record<string, string>) => renderToStaticMarkup(<LivePalette sessions={PALETTE_SESSIONS} os="windows" labels={labels} onRun={() => {}} onClose={() => {}} />);
    expect(show()).toContain("Group sessions by project");
    const h = show({ "group-by-project": "Stop grouping by project" });
    expect(h).toContain("Stop grouping by project");
    expect(h).not.toContain("Group sessions by project");
  });
});

describe("LivePalette: Esc", () => {
  const press = (key: string) => {
    const calls: string[] = [];
    escapeCloses(() => calls.push("closed"))({ key, preventDefault: () => calls.push("prevented"), stopPropagation: () => calls.push("stopped") });
    return calls;
  };
  it("closes the palette wherever the keys are, before the window hears it", () => {
    expect(press("Escape")).toEqual(["prevented", "stopped", "closed"]);
  });
  it("leaves every other key alone", () => {
    expect(press("ArrowDown")).toEqual([]);
  });
});
