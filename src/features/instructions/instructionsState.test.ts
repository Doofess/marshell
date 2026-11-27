import { describe, expect, it } from "vitest";
import type { ConfigPreview } from "../../generated/ConfigPreview";
import type { LibraryDoc } from "../../generated/LibraryDoc";
import type { LibraryView } from "../../generated/LibraryView";
import type { ProjectInfo } from "../../generated/ProjectInfo";
import { draftOf } from "./instructionsModel";
import { conflictLine, fieldOf, initialState, instructionsReducer, isDirtyNow, saveKeyActs, saveNeeded, sheetTitle, type State } from "./instructionsState";

const doc = (name: string, body = "text\n"): LibraryDoc => ({ name, scope: "user", path: `~/${name}.md`, description: "d", clis: ["claude"], globs: [], enabled: true, body, hash: "h1" });
const view = (...docs: LibraryDoc[]): LibraryView => ({ user_docs: docs.map((d) => ({ doc: d, rows: [] })), project_docs: [], user_orphans: [], project_orphans: [], out_of_date: 0, changed_outside: 0, found: ["claude"] });
const projects: ProjectInfo[] = [{ path: "C:/dev/app", name: "app", branch: null, last_used_ms: 1 }];
const opened = (): State => {
  let s = initialState(projects, "C:/dev/app");
  s = instructionsReducer(s, { type: "viewLoaded", view: view(doc("Api"), doc("Web")), project: "C:/dev/app" });
  return instructionsReducer(s, { type: "open", scope: "user", name: "Api" });
};
const dirty = (s: State): State => instructionsReducer(s, { type: "edit", draft: { ...s.draft!, body: "changed\n" } });

describe("the Instructions page's reducer", () => {
  it("starts on the project holding the folder, and on none for an unknown folder", () => {
    expect(initialState(projects, "C:/dev/app/src").project).toBe("C:/dev/app");
    expect(initialState(projects, "C:/elsewhere").project).toBeNull();
  });
  it("drops a view read for another project than the page's, so a slow first read can't hide the project's docs", () => {
    // The page reads the view before the projects are known (no project), then again on the first project; slice 8a's live
    // check saw the first answer come in last and leave the project showing "No docs yet".
    const s = initialState(projects, "C:/dev/app");
    expect(instructionsReducer(s, { type: "viewLoaded", view: view(doc("Api")), project: null }).view).toBeNull();
    expect(instructionsReducer(s, { type: "viewLoaded", view: view(doc("Api")), project: "C:/dev/app" }).view).not.toBeNull();
    const open = opened();
    const stale = instructionsReducer(open, { type: "reloaded", view: view(), project: null });
    expect(stale.view).toBe(open.view);
    expect(stale.open).toEqual(open.open);
  });
  it("opens a doc into a draft", () => {
    const s = opened();
    expect(s.open).toEqual({ scope: "user", name: "Api" });
    expect(s.draft).toEqual(draftOf(doc("Api")));
    expect(isDirtyNow(s)).toBe(false);
  });
  it("asks before opening another doc over unsaved edits, and doesn't switch", () => {
    const s = instructionsReducer(dirty(opened()), { type: "open", scope: "user", name: "Web" });
    expect(s.asking).toEqual({ kind: "open", scope: "user", name: "Web" });
    expect(s.open?.name).toBe("Api");
    expect(s.draft?.body).toBe("changed\n");
  });
  it("opens at once when nothing is unsaved", () => {
    const s = instructionsReducer(opened(), { type: "open", scope: "user", name: "Web" });
    expect(s.asking).toBeNull();
    expect(s.open?.name).toBe("Web");
  });
  it("discard goes on to the doc asked for, and cancel stays", () => {
    const asked = instructionsReducer(dirty(opened()), { type: "open", scope: "user", name: "Web" });
    const gone = instructionsReducer(asked, { type: "discard" });
    expect(gone.asking).toBeNull();
    expect(gone.open?.name).toBe("Web");
    expect(gone.draft).toEqual(draftOf(doc("Web")));
    const stay = instructionsReducer(asked, { type: "cancel" });
    expect(stay.asking).toBeNull();
    expect(stay.open?.name).toBe("Api");
    expect(stay.draft?.body).toBe("changed\n");
  });
  it("closing over unsaved edits asks, and discard closes", () => {
    const asked = instructionsReducer(dirty(opened()), { type: "closeAsked" });
    expect(asked.asking).toEqual({ kind: "close" });
    expect(asked.closed).toBe(false);
    expect(instructionsReducer(asked, { type: "discard" }).closed).toBe(true);
  });
  it("a save conflict sets the conflict line", () => {
    const s = instructionsReducer(instructionsReducer(dirty(opened()), { type: "saveStarted" }), { type: "saveConflict" });
    expect(s.conflict).toBe("Api changed on disk since you opened it.");
    expect(conflictLine("Api")).toBe("Api changed on disk since you opened it.");
    expect(s.busy).toBe(false);
  });
  it("a saved doc takes the view's copy, names itself and says so", () => {
    const saved = { ...doc("Api", "changed\n"), hash: "h2" };
    const s = instructionsReducer(dirty(opened()), { type: "saved", view: view(saved, doc("Web")), name: "Api" });
    expect(s.status).toEqual({ text: "Saved Api." });
    expect(s.draft).toEqual(draftOf(saved));
    expect(isDirtyNow(s)).toBe(false);
  });
  it("a refused save puts its sentence under the field it names", () => {
    expect(fieldOf("A doc needs a name.")).toBe("name");
    expect(fieldOf("Another doc here is named Api.")).toBe("name");
    expect(fieldOf("A description is one line.")).toBe("description");
    expect(fieldOf("/x: a glob is relative to the project, so it can't start with /.")).toBe("globs");
    expect(fieldOf("Something else.")).toBe("save");
    const s = instructionsReducer(opened(), { type: "saveRefused", message: "A doc needs a name." });
    expect(s.errors).toEqual({ name: "A doc needs a name." });
  });
  it("an apply sets the Undo status and closes the sheet, and an undo says it put the copies back", () => {
    const preview = { changeset_id: "c", files: [] } as unknown as ConfigPreview;
    let s = instructionsReducer(opened(), { type: "reviewShown", title: "Review", preview });
    expect(s.review?.preview).toBe(preview);
    s = instructionsReducer(s, { type: "applied", snapshot: "snap1" });
    expect(s.review).toBeNull();
    expect(s.status).toEqual({ text: "Copies updated.", snapshot: "snap1" });
    s = instructionsReducer(s, { type: "undone" });
    expect(s.status).toEqual({ text: "Put back." });
  });
  it("a conflict on apply keeps the sheet and says so", () => {
    const preview = { changeset_id: "c", files: [] } as unknown as ConfigPreview;
    const s = instructionsReducer(instructionsReducer(opened(), { type: "reviewShown", title: "Review", preview }), { type: "reviewConflict" });
    expect(s.review?.conflict).toBe(true);
  });
});

describe("the review sheet's titles", () => {
  it("are the plan's", () => {
    expect(sheetTitle({ kind: "review" })).toBe("Update the copies");
    expect(sheetTitle({ kind: "delete", name: "SECURITY" })).toBe("Delete SECURITY and its copies");
    expect(sheetTitle({ kind: "copy", takeCopy: false })).toBe("Put the doc's text back");
    expect(sheetTitle({ kind: "copy", takeCopy: true })).toBe("Use this copy's text");
  });
});

describe("the editor's text revision", () => {
  it("bumps when a reload brings the disk's text, and the draft holds the disk body and hash", () => {
    const s = dirty(opened());
    const disk = { ...doc("Api", "teammate\n"), hash: "h2" };
    const r = instructionsReducer(s, { type: "reloaded", view: view(disk, doc("Web")), project: "C:/dev/app" });
    expect(r.textRev).toBe(s.textRev + 1);
    expect(r.draft?.body).toBe("teammate\n");
    expect(r.draft?.hash).toBe("h2");
  });
  it("bumps when a view that a clean draft follows has a changed body", () => {
    const s = opened();
    const r = instructionsReducer(s, { type: "viewLoaded", view: view({ ...doc("Api", "adopted\n"), hash: "h2" }, doc("Web")), project: "C:/dev/app" });
    expect(r.draft?.body).toBe("adopted\n");
    expect(r.textRev).toBe(s.textRev + 1);
  });
  it("doesn't bump on an edit, so the editor stays under the cursor", () => {
    const s = opened();
    expect(dirty(s).textRev).toBe(s.textRev);
  });
  it("doesn't bump when a save's view holds the body the draft already has", () => {
    const s = dirty(opened());
    const r = instructionsReducer(s, { type: "saved", view: view(doc("Api", "changed\n"), doc("Web")), name: "Api" });
    expect(r.textRev).toBe(s.textRev);
  });
  it("bumps when a typed new draft is discarded for New", () => {
    let s = instructionsReducer(opened(), { type: "newDoc", scope: "user" });
    s = instructionsReducer(s, { type: "edit", draft: { ...s.draft!, body: "typed\n" } });
    const asked = instructionsReducer(s, { type: "newDoc", scope: "user" });
    expect(asked.asking).not.toBeNull();
    const r = instructionsReducer(asked, { type: "discard" });
    expect(r.textRev).toBe(s.textRev + 1);
  });
});
describe("Save", () => {
  it("is needed for an edited doc or a new draft, and not for an open doc with no edit", () => {
    const s = opened();
    expect(saveNeeded(s)).toBe(false);
    expect(saveNeeded(dirty(s))).toBe(true);
    const fresh = instructionsReducer(instructionsReducer(s, { type: "newDoc", scope: "user" }), { type: "discard" });
    expect(fresh.open).toBeNull();
    expect(saveNeeded(fresh)).toBe(true);
  });
  it("puts the reserved-name sentence under the name", () => {
    expect(fieldOf("A name can't be nul: Windows keeps it for a device.")).toBe("name");
  });
});
describe("Ctrl+S", () => {
  it("acts only when no sheet and no unsaved-edits question is open", () => {
    const s = opened();
    expect(saveKeyActs(s)).toBe(true);
    expect(saveKeyActs({ ...s, asking: { kind: "close" } })).toBe(false);
    expect(saveKeyActs({ ...s, review: { title: "t", preview: {} as ConfigPreview, busy: false, conflict: false } })).toBe(false);
  });
});
