import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { commandShortcut } from "../../lib/appShortcuts";
import { PALETTE_SESSIONS } from "../palette/fixtures";
import { LIVE_COMMANDS, LivePalette } from "./LivePalette";

describe("LivePalette", () => {
  it("lists only commands the window runs", () => {
    const ids = LIVE_COMMANDS.map((c) => c.id);
    for (const id of ["launcher", "close", "reopen", "rename", "move-up", "move-down", "profiles", "appearance"]) expect(ids, id).toContain(id);
    // Every other row would do nothing when chosen.
    for (const id of ids) expect(id === "profiles" || id === "appearance" || commandShortcut(id) !== null, id).toBe(true);
    for (const id of ["doctor", "history", "split", "mode-plan", "go-to-session"]) expect(ids, id).not.toContain(id);
  });
  it("shows those commands and not the inert ones", () => {
    const h = renderToStaticMarkup(<LivePalette sessions={PALETTE_SESSIONS} os="windows" onRun={() => {}} onClose={() => {}} />);
    expect(h).toContain("Shell profiles");
    expect(h).toContain("Start a session");
    expect(h).not.toContain("Open Doctor");
    expect(h).not.toContain("Split: add a pane");
  });
});
