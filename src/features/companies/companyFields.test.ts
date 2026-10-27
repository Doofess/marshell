import { describe, expect, it } from "vitest";
import { COMPANY_FIELDS } from "./companyFields";

// Copied from crates/core/src/agents/settings_view.rs, fn curated.
const CURATED = {
  claude: ["model", "effortLevel", "permissions.defaultMode", "cleanupPeriodDays", "autoUpdatesChannel", "theme"],
  codex: ["model", "model_reasoning_effort", "approval_policy", "sandbox_mode", "project_doc_max_bytes", "check_for_update_on_startup"],
  antigravity: ["model", "toolPermission", "enableTerminalSandbox", "colorScheme", "verbosity", "enableTelemetry"],
};

describe("COMPANY_FIELDS", () => {
  it.each(Object.entries(CURATED))("%s keys match the core's curated list in order", (agent, keys) => {
    expect(COMPANY_FIELDS[agent as keyof typeof COMPANY_FIELDS].map((f) => f.key)).toEqual(keys);
  });
  it("every field has a label and help", () => {
    for (const list of Object.values(COMPANY_FIELDS)) for (const f of list) expect(Boolean(f.label && f.help)).toBe(true);
  });
  it("copy", () => {
    expect(COMPANY_FIELDS.claude[3]).toEqual({ key: "cleanupPeriodDays", label: "Delete transcripts after (days)", help: "Claude deletes its own transcripts after this long. Marshell keeps its own copy." });
    expect(COMPANY_FIELDS.codex[4]?.label).toBe("AGENTS.md size limit (bytes)");
    expect(COMPANY_FIELDS.antigravity[3]?.label).toBe("Colour scheme");
    expect(COMPANY_FIELDS.antigravity[0]?.help).toBe("The model a new Antigravity session starts with, by the name /model shows, such as Gemini 3.1 Pro.");
  });
});
