import type { PDFDocumentProxy } from "pdfjs-dist";
import { describe, expect, it } from "vitest";
import { noEditor, notesBlocked } from "./blocked";

const SIGNED = "This PDF is signed, and a note would break its signature.";
const OWNER = "This PDF's owner doesn't allow notes.";
const XFA = "This PDF is an XFA form.";

// As pdf.js 6 gives them: a signature field's value is always null, and `getSignatures` lists only the fields that hold a
// signature (a /Sig value whose /ByteRange and /Contents check out), or null.
const sigField = { s: [{ type: "signature", value: null }] };
const fake = (o: { permissions?: number[] | null; fields?: Record<string, object[]> | null; signatures?: object[] | null; xfa?: boolean }) =>
  ({
    getPermissions: async () => (o.permissions ? new Set(o.permissions) : null),
    getFieldObjects: async () => (o.fields ? new Map(Object.entries(o.fields)) : null),
    getSignatures: async () => o.signatures ?? null,
    isPureXfa: o.xfa ?? false,
  }) as unknown as PDFDocumentProxy;

describe("notesBlocked", () => {
  it("notesBlocked_permissions_without_the_flag", async () => expect(await notesBlocked(fake({ permissions: [4] }), false, "a.pdf")).toBe(OWNER));
  it("notesBlocked_permissions_with_the_flag", async () => expect(await notesBlocked(fake({ permissions: [4, 32] }), false, "a.pdf")).toBeNull());
  it("notesBlocked_unencrypted", async () => expect(await notesBlocked(fake({}), false, "a.pdf")).toBeNull());
  it("notesBlocked_signed", async () => expect(await notesBlocked(fake({ fields: sigField, signatures: [{ id: "4R:0-10-20-30" }] }), false, "a.pdf")).toBe(SIGNED));
  it("notesBlocked_a_blank_signature_field", async () => expect(await notesBlocked(fake({ fields: sigField, signatures: null }), false, "a.pdf")).toBeNull());
  it("notesBlocked_signatures_unreadable_is_not_signed", async () => {
    const d = { ...fake({ fields: sigField }), getSignatures: async () => Promise.reject(new Error("bad")) } as unknown as PDFDocumentProxy;
    expect(await notesBlocked(d, false, "a.pdf")).toBeNull();
  });
  it("notesBlocked_xfa", async () => expect(await notesBlocked(fake({ xfa: true }), false, "a.pdf")).toBe(XFA));
  it("notesBlocked_read_only", async () => expect(await notesBlocked(fake({}), true, "a.pdf")).toBe("a.pdf is read-only."));
  it("notesBlocked_signed_comes_before_owner_xfa_and_read_only", async () => {
    const d = fake({ permissions: [4], fields: sigField, signatures: [{ id: "4R:0-10-20-30" }], xfa: true });
    expect(await notesBlocked(d, true, "a.pdf")).toBe(SIGNED);
    expect(await notesBlocked(fake({ permissions: [4], xfa: true }), true, "a.pdf")).toBe(OWNER);
    expect(await notesBlocked(fake({ xfa: true }), true, "a.pdf")).toBe(XFA);
  });
});

describe("final review 6q-6: a read-only file's notes", () => {
  const RO = "a.pdf is read-only.";
  it("makes no editor before the reason is known, for a reason from the document, or for a read-only file with its notes saved", () => {
    expect(noEditor(undefined, "a.pdf", false)).toBe(true);
    expect(noEditor(SIGNED, "a.pdf", false)).toBe(true);
    expect(noEditor(SIGNED, "a.pdf", true)).toBe(true);
    // Notes are off for a read-only file: its own notes can't be moved, recoloured or deleted either.
    expect(noEditor(RO, "a.pdf", false)).toBe(true);
  });
  it("keeps the editor for a PDF that takes notes, and for a read-only file while unsaved notes are on screen", () => {
    expect(noEditor(null, "a.pdf", false)).toBe(false);
    expect(noEditor(null, "a.pdf", true)).toBe(false);
    expect(noEditor(RO, "a.pdf", true)).toBe(false);
  });
});
