import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import type { OwnAgents } from "../../generated/OwnAgents";
import { OwnAgentsView, type OwnAgentsViewProps } from "./OwnAgentsSection";
import { blankForm, buildManifest } from "./ownAgents";

const none = () => {};
const handlers = { onAdd: none, onEdit: none, onRemove: none, onChange: none, onSave: none, onCancel: none, onReload: none };
const empty: OwnAgents = { agents: [], problems: [], version: "" };
const view = (p: Partial<OwnAgentsViewProps> = {}) => renderToStaticMarkup(<OwnAgentsView own={empty} editing={null} {...handlers} {...p} />);

describe("OwnAgentsView", () => {
  it("shows the empty state with Add an agent", () => {
    const h = view();
    expect(h).toContain("No agents of your own yet.");
    expect(h).toMatch(/<button[^>]*>Add an agent<\/button>/);
  });
  it("lists the file's own problems at the top with the reason", () => {
    const h = view({ own: { agents: [], version: "", problems: [{ index: 0, id: null, reason: "agents.json can't be read: access denied" }] } });
    expect(h).toContain("agents.json can&#x27;t be read: access denied");
  });
  it("lists an entry with Edit and Remove named for it", () => {
    const m = buildManifest({ ...blankForm(), name: "My Agent", command: "my" });
    const h = view({ own: { agents: [m], problems: [{ index: 0, id: "my-agent", reason: "Its command was not found." }], version: "v" } });
    expect(h).toContain("My Agent");
    expect(h).toContain('aria-label="Edit My Agent"');
    expect(h).toContain('aria-label="Remove My Agent"');
    expect(h).toContain("Its command was not found.");
  });
  it("gives every form field a label, in keyboard order", () => {
    const h = view({ editing: { index: null, form: { ...blankForm(), name: "My Agent" }, busy: false } });
    const labels = ["Name", "Command", "First prompt", "Continue flags", "Manual flags", "Plan flags", "Auto-edit flags", "Full auto flags", "Bypass flags", "Program names to recognise"];
    let at = -1;
    for (const l of labels) {
      const i = h.indexOf(`>${l}<`);
      expect(i, l).toBeGreaterThan(at);
      at = i;
    }
    expect(h).toContain("Id: my-agent");
    expect(h).not.toContain('tabindex="');
    expect(h).toContain(">Save</button>");
    expect(h).toContain(">Cancel</button>");
  });
  it("ties a refusal to its field with aria-invalid and aria-describedby", () => {
    const h = view({ editing: { index: null, form: blankForm(), busy: false, problem: "Its command is empty." } });
    expect(h).toMatch(/<input[^>]*aria-invalid="true"[^>]*aria-describedby="own-agent-error[ "]/);
    expect(h).toMatch(/id="own-agent-error"[^>]*>Its command is empty\./);
  });
  it("shows the reload copy and button for a 409", () => {
    const h = view({ editing: { index: null, form: blankForm(), busy: false }, conflict: true });
    expect(h).toContain("agents.json changed since you opened it. Reload to see the change.");
    expect(h).toMatch(/<button[^>]*>Reload<\/button>/);
  });
});
