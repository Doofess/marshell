import type { PDFDocumentProxy } from "pdfjs-dist";
import { describe, expect, it } from "vitest";
import { notesBlocked } from "./blocked";

const SIGNED = "This PDF is signed, and a note would break its signature.";
const OWNER = "This PDF's owner doesn't allow notes.";
const XFA = "This PDF is an XFA form.";

const fake = (o: { permissions?: number[] | null; fields?: Record<string, object[]> | null; xfa?: boolean }) =>
  ({
    getPermissions: async () => (o.permissions ? new Set(o.permissions) : null),
    getFieldObjects: async () => (o.fields ? new Map(Object.entries(o.fields)) : null),
    isPureXfa: o.xfa ?? false,
  }) as unknown as PDFDocumentProxy;

describe("notesBlocked", () => {
  it("notesBlocked_permissions_without_the_flag", async () => expect(await notesBlocked(fake({ permissions: [4] }), false, "a.pdf")).toBe(OWNER));
  it("notesBlocked_permissions_with_the_flag", async () => expect(await notesBlocked(fake({ permissions: [4, 32] }), false, "a.pdf")).toBeNull());
  it("notesBlocked_unencrypted", async () => expect(await notesBlocked(fake({}), false, "a.pdf")).toBeNull());
  it("notesBlocked_signed_with_a_value", async () => expect(await notesBlocked(fake({ fields: { s: [{ type: "signature", value: "x" }] } }), false, "a.pdf")).toBe(SIGNED));
  it("notesBlocked_a_blank_signature_field", async () => expect(await notesBlocked(fake({ fields: { s: [{ type: "signature", value: "" }] } }), false, "a.pdf")).toBeNull());
  it("notesBlocked_xfa", async () => expect(await notesBlocked(fake({ xfa: true }), false, "a.pdf")).toBe(XFA));
  it("notesBlocked_read_only", async () => expect(await notesBlocked(fake({}), true, "a.pdf")).toBe("a.pdf is read-only."));
  it("notesBlocked_signed_comes_before_owner_xfa_and_read_only", async () => {
    const d = fake({ permissions: [4], fields: { s: [{ type: "signature", value: "x" }] }, xfa: true });
    expect(await notesBlocked(d, true, "a.pdf")).toBe(SIGNED);
    expect(await notesBlocked(fake({ permissions: [4], xfa: true }), true, "a.pdf")).toBe(OWNER);
    expect(await notesBlocked(fake({ xfa: true }), true, "a.pdf")).toBe(XFA);
  });
});
