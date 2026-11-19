import type { PDFDocumentProxy } from "pdfjs-dist";

/** pdf.js's `PermissionFlag.MODIFY_ANNOTATIONS`, the bit of an encrypted PDF's permissions that allows notes. */
const MODIFY_ANNOTATIONS = 0x20;

/** What a reason comes from: the document (no editor is made for it) or only the file's state (the editor stays, with its tool off). */
export const FROM_FILE = (name: string) => `${name} is read-only.`;

/**
 * Why a PDF can't take notes, or null when it can. Several may apply; the first of these is given: a signature (a note would
 * break it), the owner's permissions, an XFA form, and a file that is read-only on disk.
 */
export async function notesBlocked(doc: PDFDocumentProxy, readOnly: boolean, name: string): Promise<string | null> {
  // pdf.js gives every signature field the value null; `getSignatures` lists the fields that hold a signature, so a blank
  // signature field doesn't count.
  const signatures = await doc.getSignatures().catch(() => null);
  if (signatures && signatures.length > 0) return "This PDF is signed, and a note would break its signature.";
  const allowed = await doc.getPermissions().catch(() => null);
  if (allowed && !allowed.has(MODIFY_ANNOTATIONS)) return "This PDF's owner doesn't allow notes.";
  if (doc.isPureXfa) return "This PDF is an XFA form.";
  if (readOnly) return FROM_FILE(name);
  return null;
}

/** Whether a reason comes from the document itself, so no editor is made for it (a read-only file keeps its editor and its unsaved notes). */
export function fromDocument(reason: string | null, name: string): boolean {
  return reason !== null && reason !== FROM_FILE(name);
}
