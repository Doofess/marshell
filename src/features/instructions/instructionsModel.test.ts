import { describe, expect, it } from "vitest";
import type { CopyRow } from "../../generated/CopyRow";
import type { LibraryDoc } from "../../generated/LibraryDoc";
import { barText, copilotLocked, copyFixes, copyFixOf, draftOf, isDirty, newDraft, rowLabel, stateText, toggleCli, toSave } from "./instructionsModel";

const doc: LibraryDoc = { name: "Api", scope: "project", path: "docs/instructions/Api.md", description: "d", clis: ["claude", "copilot"], globs: ["src/**", "*.ts"], enabled: true, body: "text\n", hash: "h1" };
const row = (o: Partial<CopyRow>): CopyRow => ({ state: "in_sync", notes: [], shared: false, ...o });

describe("barText", () => {
  it("covers the table", () => {
    expect(barText(0, 0)).toBeNull();
    expect(barText(1, 0)).toBe("1 copy is out of date");
    expect(barText(4, 0)).toBe("4 copies are out of date");
    expect(barText(4, 1)).toBe("4 copies are out of date. 1 copy was changed outside Marshell; its row has the fixes.");
    expect(barText(0, 2)).toBe("2 copies were changed outside Marshell; their rows have the fixes.");
  });
});

describe("stateText", () => {
  it("covers every state", () => {
    const want = {
      in_sync: "Up to date",
      out_of_date: "Out of date",
      missing: "No copy yet",
      changed_outside: "Changed outside Marshell",
      extra: "Review removes this copy",
      taken: "Not Marshell's file",
      no_copy: "No copy",
      waiting: "Waiting for the distro",
      refused: "Marshell can't write this copy",
    };
    for (const [s, t] of Object.entries(want)) expect(stateText(row({ state: s as CopyRow["state"] }))).toBe(t);
  });
});

describe("toggleCli", () => {
  it("project: claude on ticks copilot, copilot stays on while claude is", () => {
    const d = newDraft("project", []);
    const off = { ...d, clis: ["codex"] };
    expect(toggleCli(off, "claude").clis).toEqual(["claude", "codex", "copilot"]);
    const both = { ...d, clis: ["claude", "copilot"] };
    expect(toggleCli(both, "copilot")).toBe(both);
    expect(copilotLocked(both)).toBe(true);
    expect(toggleCli(both, "claude").clis).toEqual(["copilot"]);
    expect(copilotLocked(toggleCli(both, "claude"))).toBe(false);
  });
  it("user: the two are independent", () => {
    const d = { ...newDraft("user", []), clis: ["claude", "copilot"] };
    expect(copilotLocked(d)).toBe(false);
    expect(toggleCli(d, "copilot").clis).toEqual(["claude"]);
    expect(toggleCli({ ...d, clis: [] }, "claude").clis).toEqual(["claude"]);
  });
});

describe("drafts", () => {
  it("newDraft", () => {
    expect(newDraft("user", ["claude", "qwen"]).clis).toEqual(["claude", "qwen"]);
    expect(newDraft("project", []).clis.length).toBe(5);
    expect(newDraft("user", []).hash).toBeNull();
  });
  it("isDirty", () => {
    const d = draftOf(doc);
    expect(isDirty(d, doc)).toBe(false);
    expect(isDirty({ ...d, body: "x" }, doc)).toBe(true);
    expect(isDirty(newDraft("user", []), null)).toBe(false);
    expect(isDirty({ ...newDraft("user", []), name: "a" }, null)).toBe(true);
  });
  it("toSave splits globs by lines and trims", () => {
    const s = toSave({ ...draftOf(doc), globsText: " src/** \n\n*.ts\r\n" }, "C:/p");
    expect(s.globs).toEqual(["src/**", "*.ts"]);
    expect(s.project).toBe("C:/p");
    expect(s.expected_hash).toBe("h1");
    // An existing doc names itself even when its name stays, or the core takes it for a new doc of a taken name.
    expect(s.old_name).toBe("Api");
    expect("old_name" in toSave(newDraft("user", []), null)).toBe(false);
    const r = toSave({ ...draftOf(doc), name: "Web" }, null);
    expect(r.old_name).toBe("Api");
    expect("project" in r).toBe(false);
    expect("expected_hash" in toSave(newDraft("user", []), null)).toBe(false);
  });
});

describe("rows", () => {
  it("rowLabel", () => {
    expect(rowLabel(row({ cli: "codex" }))).toBe("Codex");
    expect(rowLabel(row({ cli: "qwen", distro: "Ubuntu" }))).toBe("Qwen Code in Ubuntu");
    expect(rowLabel(row({ state: "waiting", distro: "Ubuntu" }))).toBe("Ubuntu");
  });
  it("copyFixes", () => {
    expect(copyFixes(row({ state: "changed_outside", fixes: true }), false)).toBe(true);
    expect(copyFixes(row({ state: "changed_outside", fixes: true }), true)).toBe(false);
    expect(copyFixes(row({ state: "changed_outside" }), false)).toBe(false);
    expect(copyFixes(row({ state: "in_sync" }), false)).toBe(false);
  });
  it("copyFixOf", () => {
    expect(copyFixOf(doc, row({ cli: "codex", distro: "Ubuntu" }), "C:/p", true)).toEqual({ scope: "project", project: "C:/p", name: "Api", cli: "codex", distro: "Ubuntu", take_copy: true });
    const f = copyFixOf({ ...doc, scope: "user" }, row({ cli: "claude" }), null, false);
    expect(f).toEqual({ scope: "user", name: "Api", cli: "claude", take_copy: false });
  });
});
