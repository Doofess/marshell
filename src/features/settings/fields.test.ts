import { describe, expect, it } from "vitest";
import { appliesNote, isEditable, type SettingField } from "./fields";

const f = (o: Partial<SettingField> = {}): SettingField => ({ key: "theme", label: "Theme", help: "", kind: "enum", scope: "user", applies: "live", category: "general", ...o });

describe("isEditable", () => {
  it("allows an ordinary field", () => expect(isEditable(f())).toEqual({ ok: true }));
  it("locks a managed field and says who set it", () => {
    expect(isEditable(f({ scope: "managed" }))).toEqual({ ok: false, reason: "Set by your organisation. Marshell can't change it." });
    expect(isEditable(f({ lockedBy: "managed" })).ok).toBe(false);
  });
  it("locks everything in a read-only or unparsable file, with the reason", () => {
    expect(isEditable(f(), { readOnly: true })).toEqual({ ok: false, reason: "This file is read-only, so Marshell can't save changes." });
    expect(isEditable(f(), { unparsable: true })).toEqual({ ok: false, reason: "This file does not parse, so Marshell won't edit it." });
  });
});
describe("appliesNote", () => {
  it("says when a change takes effect", () => {
    expect(appliesNote(f({ applies: "live" }))).toBe("Applies at once.");
    expect(appliesNote(f({ applies: "new-sessions" }))).toBe("Applies to new sessions.");
    expect(appliesNote(f({ applies: "restart" }))).toBe("Applies after you restart the session.");
  });
});
