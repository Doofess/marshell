import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import type { CopyRow } from "../../generated/CopyRow";
import type { DocView } from "../../generated/DocView";
import type { LibraryView } from "../../generated/LibraryView";
import { DOCTOR_PREVIEW } from "../doctor/fixtures";
import { EDITOR_DEFAULTS } from "../editor/editorPrefs";
import { CopiesReview } from "./CopiesReview";
import { InstructionsPage } from "./InstructionsPage";
import { type Draft, newDraft } from "./instructionsModel";

const noop = () => {};
const row = (over: Partial<CopyRow> = {}): CopyRow => ({ cli: "claude", path: ".claude/rules/marshell-style.md", state: "in_sync", notes: [], shared: false, ...over });
const docView = (name: string, rows: CopyRow[] = [row()]): DocView => ({
  doc: { name, scope: "project", path: `docs/instructions/${name}.md`, description: "How we write", clis: ["claude", "copilot"], globs: [], enabled: true, body: "Be brief.", hash: "abc" },
  rows,
});
const view = (over: Partial<LibraryView> = {}): LibraryView => ({ project: "C:\\dev\\app", user_docs: [], project_docs: [], user_orphans: [], project_orphans: [], out_of_date: 0, changed_outside: 0, found: ["claude"], ...over });
const draft = (over: Partial<Draft> = {}): Draft => ({ ...newDraft("project", []), name: "style", oldName: "style", ...over });

type Props = Parameters<typeof InstructionsPage>[0];
const page = (over: Partial<Props> = {}) =>
  renderToStaticMarkup(
    <InstructionsPage
      view={view()}
      projects={[{ path: "C:\\dev\\app", name: "app", branch: null, last_used_ms: 0 }]}
      project="C:\\dev\\app"
      open={null}
      draft={null}
      errors={{}}
      busy={false}
      status={null}
      prefs={EDITOR_DEFAULTS}
      appChord={() => false}
      onPickProject={noop}
      onOpen={noop}
      onNew={noop}
      onDraft={noop}
      onSave={noop}
      onDelete={noop}
      onReview={noop}
      onCopyFix={noop}
      onCheckAgain={noop}
      onClose={noop}
      editorRef={null}
      {...over}
    />,
  );
const opened = (d: Draft, rows: CopyRow[], over: Partial<Props> = {}) =>
  page({ view: view({ project_docs: [docView("style", rows)] }), open: { scope: d.scope, name: "style" }, draft: d, ...over });

const PUT_BACK = "Put the doc&#x27;s text back";
const USE_COPY = "Use this copy&#x27;s text";

describe("InstructionsPage", () => {
  it("shows the bar's text and Review for 4 out-of-date copies, and no bar for 0 and 0", () => {
    const h = page({ view: view({ out_of_date: 4 }) });
    expect(h).toContain("4 copies are out of date");
    expect(h).toContain(">Review<");
    const none = page();
    expect(none).not.toContain(">Review<");
    expect(none).toContain("Check again");
    expect(none).toContain('aria-label="Close Instructions"');
  });
  it("says Copies updated with Undo after an apply", () => {
    const h = page({ status: { text: "Copies updated.", undo: noop } });
    expect(h).toContain("Copies updated.");
    expect(h).toContain(">Undo<");
  });
  it("says No docs yet. for an empty user list, and the no-project sentence", () => {
    const h = page({ project: null, projects: [], view: view({ project: undefined }) });
    expect(h).toContain("Every project");
    expect(h).toContain("No docs yet.");
    expect(h).toContain("New doc for every project");
    expect(h).toContain("No project yet. Open a session in a folder, or add one in Settings → Projects.");
  });
  it("lists docs as buttons with aria-current on the open one, and the project picker", () => {
    const h = opened(draft(), [row()]);
    expect(h).toMatch(/aria-current="true"[^>]*>style</);
    expect(h).toContain('aria-label="Project"');
    expect(h).toContain("New doc for this project");
  });
  it("renders a project_note and no New doc for this project", () => {
    const h = page({ view: view({ project_note: "Ubuntu is stopped." }) });
    expect(h).toContain("Ubuntu is stopped.");
    expect(h).not.toContain("New doc for this project");
  });
  it("renders both fix buttons for a changed_outside row with fixes, and none for in_sync, an orphan, or a row without fixes", () => {
    const fix = opened(draft(), [row({ state: "changed_outside", fixes: true })]);
    expect(fix).toContain(PUT_BACK);
    expect(fix).toContain(USE_COPY);
    expect(fix).toContain("Takes the text, not the frontmatter.");
    for (const rows of [[row()], [row({ state: "changed_outside" })]]) {
      const h = opened(draft(), rows);
      expect(h).not.toContain(PUT_BACK);
      expect(h).not.toContain(USE_COPY);
    }
    const orphan = page({ view: view({ project_orphans: [row({ state: "changed_outside", fixes: true })] }) });
    expect(orphan).not.toContain(PUT_BACK);
  });
  it("renders Copies with no doc with an orphan", () => {
    expect(page({ view: view({ project_orphans: [row({ state: "taken" })] }) })).toContain("Copies with no doc");
    expect(page()).not.toContain("Copies with no doc");
  });
  it("shows a shared Copilot CLI row's note", () => {
    const h = opened(draft(), [row({ cli: "copilot", shared: true, notes: ["Reads Claude Code's copy."] })]);
    expect(h).toContain("Copilot CLI");
    expect(h).toContain("Reads Claude Code&#x27;s copy.");
  });
  it("renders the Copilot CLI box disabled and checked in a project draft with claude ticked", () => {
    const h = opened(draft({ clis: ["claude", "copilot"] }), []);
    expect(h).toMatch(/<input(?=[^>]*type="checkbox")(?=[^>]*checked)(?=[^>]*disabled)[^>]*><span[^>]*>Copilot CLI/);
    expect(h).toContain("Copilot CLI reads .claude/rules, so it loads this too.");
  });
  it("renders no On in this project for a user draft", () => {
    const h = opened(draft({ scope: "user" }), []);
    expect(h).not.toContain("On in this project");
    expect(opened(draft(), [])).toContain("On in this project");
  });
  it("ties a name error to its field with aria-describedby", () => {
    const h = opened(draft(), [], { errors: { name: "A doc named style already exists." } });
    const id = /id="([^"]+)"[^>]*>A doc named style already exists\./.exec(h)?.[1];
    expect(id).toBeDefined();
    expect(h).toMatch(new RegExp(`<input[^>]*aria-describedby="${id}"`));
  });
  it("has the form's labels and buttons", () => {
    const h = opened(draft(), []);
    for (const t of ["Name", "Description", "CLIs", "Globs", "Text", "Save", "Ctrl+S", "Delete…", "Loads in", "One per line, relative to the project. Empty: the doc always loads."]) expect(h).toContain(t);
  });
});

describe("CopiesReview", () => {
  const sheet = (over: Partial<Parameters<typeof CopiesReview>[0]> = {}) =>
    renderToStaticMarkup(<CopiesReview title="Update the copies" preview={DOCTOR_PREVIEW} busy={false} conflict={false} onApply={noop} onCancel={noop} onReload={noop} {...over} />);
  it("is a modal dialog with its title and a heading that takes focus", () => {
    const h = sheet();
    expect(h).toContain('aria-modal="true"');
    expect(h).toContain("Update the copies");
    expect(h).toMatch(/<h2[^>]*tabindex="-1"/);
    expect(h).toContain(">Apply<");
    expect(h).toContain(">Cancel<");
  });
  it("offers Reload and no Apply on a conflict", () => {
    const h = sheet({ conflict: true });
    expect(h).toContain("A file changed since this preview. Reload it to see the change.");
    expect(h).toContain(">Reload<");
    expect(h).not.toContain(">Apply<");
  });
  it("says there is nothing to update and offers only Close", () => {
    const h = sheet({ preview: { ...DOCTOR_PREVIEW, files: [] } });
    expect(h).toContain("Nothing to update: the copies are up to date.");
    expect(h).toContain(">Close<");
    expect(h).not.toContain(">Apply<");
    expect(h).not.toContain(">Cancel<");
  });
});
