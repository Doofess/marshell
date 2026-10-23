import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { COMMANDS } from "../../lib/keymap";
import { ShortcutSheet, sheetGroups } from "./ShortcutSheet";

const html = (os: "windows" | "mac" | "linux") => renderToStaticMarkup(<ShortcutSheet os={os} />);

describe("ShortcutSheet", () => {
  it("lists every command that has a key, with the key written for the system", () => {
    const w = html("windows");
    const m = html("mac");
    for (const c of COMMANDS.filter((c) => c.binding && !c.id.startsWith("focus-session-"))) {
      const escaped = c.label.replace(/'/g, "&#x27;");
      expect(w, c.id).toContain(escaped);
      expect(m, c.id).toContain(escaped);
    }
    expect(w).toContain("Ctrl+Shift+K");
    expect(m).toContain("⌘K");
    expect(m).not.toContain("Ctrl+Shift+K");
  });
  it("shows the session keys once, as a range, not nine rows", () => {
    const w = html("windows");
    expect(w).toContain("Go to session 1 to 9");
    expect(w).toContain("Ctrl+Shift+1 to 9");
    expect(html("mac")).toContain("⌘1 to 9");
    expect(w).not.toContain("Go to session 4<");
  });
  it("teaches the navigation layer in its own words and says plain keys still reach the terminal", () => {
    const w = html("windows");
    expect(w).toContain("Hold Ctrl+Shift, then press Tab or an arrow key. Release to select.");
    expect(w).toContain("Plain Tab, Esc and the arrow keys always go to the program in the terminal.");
    expect(html("mac")).toContain("Hold ⌃⇧, then press Tab or an arrow key.");
  });
  it("puts navigation first, and writes its chords with the navigation modifier", () => {
    const groups = sheetGroups("windows").map((g) => g.title);
    expect(groups).toEqual(["Navigation", "Sessions", "View", "Tools"]);
    const w = html("windows");
    expect(w).toContain("Select the next terminal on screen");
    expect(w).toContain("Ctrl+Shift+Tab");
    expect(w).toContain("Ctrl+Shift+Backspace");
    expect(w).toContain("Ctrl+Shift+←↑↓→");
    expect(html("mac")).toContain("⌃⇧Tab");
  });
  it("gives the whole sheet an accessible name and each group a heading", () => {
    const w = html("linux");
    const id = /aria-labelledby="([^"]+)"/.exec(w)![1]!;
    expect(w).toContain(`id="${id}"`);
    expect((w.match(/<h3/g) ?? []).length).toBe(4);
  });
  it("names the navigation modifier that is set", () => {
    expect(renderToStaticMarkup(<ShortcutSheet os="linux" nav="ctrl+alt" />)).toContain("Hold Ctrl+Alt, then");
  });
  it("lists only the commands that run, when told which do", () => {
    const off = ["history", "find", "global-search", "next-waiting"];
    const labels = sheetGroups("windows", undefined, (id) => !off.includes(id)).flatMap((g) => g.rows.map((r) => r.label));
    for (const id of off) expect(labels).not.toContain(COMMANDS.find((c) => c.id === id)!.label);
    expect(labels).toContain("Command palette");
    expect(labels).toContain("Keyboard shortcuts");
    expect(labels).toContain("Go to session 1 to 9");
  });
  it("drops a group with nothing left in it, and the rows that stand for commands that do not run", () => {
    const groups = sheetGroups("windows", undefined, (id) => COMMANDS.find((c) => c.id === id)?.group === "Tools");
    expect(groups.map((g) => g.title)).toEqual(["Tools"]);
    const none = sheetGroups("windows", undefined, () => false);
    expect(none).toEqual([]);
  });
});
