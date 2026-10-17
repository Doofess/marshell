import { pdfErrorMessage } from "./pdfjs";

/**
 * What a failed load does to the view. A file that is being regenerated can be fetched half-written; if a document is
 * already shown, it stays (scroll position, a marked box) and the next version is waited for. Only a view with nothing
 * loaded yet shows the error.
 */
export function afterLoadFailure(documentShown: boolean, e: unknown): { keepDocument: boolean; error: string | null } {
  return documentShown ? { keepDocument: true, error: null } : { keepDocument: false, error: pdfErrorMessage(e) };
}
