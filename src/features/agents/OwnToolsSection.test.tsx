import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import type { OwnTools } from "../../generated/OwnTools";
import type { ToolProfile } from "../../generated/ToolProfile";
import { OwnToolsView, type OwnToolsViewProps } from "./OwnToolsSection";
import { blankDraft, draftOf, focusKeys } from "./ownTools";

const none = () => {};
const handlers = { onAdd: none, onEdit: none, onRemove: none, onConfirmRemove: none, onCancelRemove: none, onChange: none, onSave: none, onCancel: none, onReload: none };
const tool: ToolProfile = {
  id: "my-tool",
  name: "My Tool",
  commands: ["mytool"],
  scripts: [],
  launcher: false,
  needs_you: [{ pattern: "^ok\\?", phrase: "wants a yes", kind: "text" }],
  ports: [{ pattern: "on (\\S+)" }],
  quiet_ports: [],
  presets: [{ id: "go", name: "Go", program: "mytool", args: ["{where}"], fields: [{ id: "where", label: "Where" }] }],
};
const own: OwnTools = { tools: [tool], problems: [], version: "v" };
const view = (p: Partial<OwnToolsViewProps> = {}) => renderToStaticMarkup(<OwnToolsView own={own} editing={null} removing={null} {...handlers} {...p} />);

describe("OwnToolsView", () => {
  it("shows the empty state with Add your own tool", () => {
    const h = view({ own: { tools: [], problems: [], version: "" } });
    expect(h).toContain("No tools of your own yet.");
    expect(h).toMatch(/<button[^>]*>Add your own tool<\/button>/);
  });
  it("shows a problem beside its entry with its reason", () => {
    const h = view({ own: { ...own, problems: [{ index: 0, id: "my-tool", reason: 'The id "my-tool" is taken by a shipped tool.' }] } });
    expect(h.indexOf("is taken by a shipped tool.")).toBeGreaterThan(h.indexOf("My Tool"));
    expect(h).toContain('aria-label="Edit My Tool"');
    expect(h).toContain('aria-label="Remove My Tool"');
  });
  it("after a 422 each problem shows on its entry", () => {
    const h = view({ problems: [{ index: 0, id: "my-tool", reason: "Its pattern is not a regular expression." }] });
    expect(h).toContain("Its pattern is not a regular expression.");
  });
  it("after a 409 shows the reload copy and keeps the draft", () => {
    const h = view({ editing: { index: 0, draft: { ...draftOf(tool), name: "Renamed tool" }, busy: false }, conflict: true });
    expect(h).toContain("tools.json changed since you opened it. Reload to see it.");
    expect(h).toMatch(/<button[^>]*>Reload<\/button>/);
    expect(h).toContain('value="Renamed tool"');
  });
  it("asks Remove <name>? with Remove and Cancel", () => {
    const h = view({ removing: 0 });
    expect(h).toContain("Remove My Tool?");
    expect(h).toMatch(/<button[^>]*>Remove<\/button>/);
    expect(h).toMatch(/<button[^>]*>Cancel<\/button>/);
  });
  it("gives the form labelled fields, the id read-only and a label input per placeholder", () => {
    const h = view({ editing: { index: null, draft: { ...draftOf(tool), name: "My Tool" }, busy: false } });
    for (const l of ["Name", "Commands", "Needs-you patterns", "Port lines", "Launch presets", "Program", "Arguments, one per line", "Label for where", "Phrase", "Kind"]) expect(h, l).toContain(l);
    expect(h).toContain("Id: my-tool");
    expect(h).toContain(">Text<");
    expect(h).toContain(">Browser<");
    expect(h).toContain(">Save</button>");
    expect(h).toContain(">Cancel</button>");
    expect(blankDraft().presets).toEqual([]);
  });
  it("ties a refusal to the form with aria-invalid", () => {
    const h = view({ editing: { index: null, draft: blankDraft(), busy: false, problem: "It needs a command." } });
    expect(h).toMatch(/<input[^>]*aria-invalid="true"[^>]*aria-describedby="own-tool-error[ "]/);
    expect(h).toMatch(/id="own-tool-error"[^>]*>It needs a command\./);
  });
});

describe("an own tool that could not be read", () => {
  const SHAPE = "It has an unexpected shape (missing field `commands`).";
  const broken = (reason: string) => view({ own: { ...own, problems: [{ index: 0, id: "my-tool", reason }] } });
  it("offers Remove but not Edit, and says where to fix it", () => {
    const h = broken(SHAPE);
    expect(h).toContain(SHAPE);
    expect(h).toContain("Fix this one in tools.json, or remove it.");
    expect(h.indexOf("Fix this one")).toBeGreaterThan(h.indexOf("unexpected shape"));
    expect(h).not.toContain('aria-label="Edit My Tool"');
    expect(h).not.toContain('data-own-focus="edit-0"');
    expect(h).toContain('aria-label="Remove My Tool"');
    expect(h).toContain('data-own-focus="remove-0"');
  });
  it("keeps Edit for an entry that parsed but failed validation", () => {
    const h = broken("Its pattern is not a regular expression.");
    expect(h).toContain('aria-label="Edit My Tool"');
    expect(h).toContain('aria-label="Remove My Tool"');
    expect(h).not.toContain("Fix this one in tools.json");
  });
  it("keeps Edit on the entries beside it", () => {
    const two: OwnTools = { tools: [tool, { ...tool, id: "other", name: "Other" }], problems: [{ index: 0, id: "my-tool", reason: SHAPE }], version: "v" };
    const h = view({ own: two });
    expect(h).not.toContain('aria-label="Edit My Tool"');
    expect(h).toContain('aria-label="Edit Other"');
  });
});

describe("focusKeys", () => {
  it("names the Edit button first and the Remove button of the same row as the fallback, so a row without Edit still takes the focus", () => {
    expect(focusKeys({ to: "edit", index: 2 })).toEqual(["edit-2", "remove-2"]);
    expect(focusKeys({ to: "add" })).toEqual(["add"]);
  });
});
