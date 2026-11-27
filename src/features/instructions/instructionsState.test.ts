import { describe, expect, it } from "vitest";
import type { ConfigPreview } from "../../generated/ConfigPreview";
import type { LibraryDoc } from "../../generated/LibraryDoc";
import type { LibraryView } from "../../generated/LibraryView";
import type { ProjectInfo } from "../../generated/ProjectInfo";
import { draftOf } from "./instructionsModel";
import { conflictLine, fieldOf, initialState, instructionsReducer, isDirtyNow, type State } from "./instructionsState";

const doc = (name: string, body = "text\n"): LibraryDoc => ({ name, scope: "user", path: `~/${name}.md`, description: "d", clis: ["claude"], globs: [], enabled: true, body, hash: "h1" });
const view = (...docs: LibraryDoc[]): LibraryView => ({ user_docs: docs.map((d) => ({ doc: d, rows: [] })), project_docs: [], user_orphans: [], project_orphans: [], out_of_date: 0, changed_outside: 0, found: ["claude"] });
const projects: ProjectInfo[] = [{ path: "C:/dev/app", name: "app", branch: null, last_used_ms: 1 }];
const opened = (): State => {
  let s = initialState(projects, "C:/dev/app");
  s = instructionsReducer(s, { type: "viewLoaded", view: view(doc("Api"), doc("Web")) });
  return instructionsReducer(s, { type: "open", scope: "user", name: "Api" });
};
const dirty = (s: State): State => instructionsReducer(s, { type: "edit", draft: { ...s.draft!, body: "changed\n" } });

describe("the Instructions page's reducer", () => {
  it("starts on the project holding the folder, and on none for an unknown folder", () => {
    expect(initialState(projects, "C:/dev/app/src").project).toBe("C:/dev/app");
    expect(initialState(projects, "C:/elsewhere").project).toBeNull();
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
