import type { ConfigPreview } from "../../generated/ConfigPreview";
import type { DocScope } from "../../generated/DocScope";
import type { LibraryDoc } from "../../generated/LibraryDoc";
import type { LibraryView } from "../../generated/LibraryView";
import type { ProjectInfo } from "../../generated/ProjectInfo";
import { pickProject } from "../settings/projectsDoc";
import type { FormErrors } from "./DocForm";
import type { OpenDoc } from "./DocList";
import { type Draft, draftOf, isDirty, newDraft } from "./instructionsModel";

/** What the person asked for while the draft had unsaved edits; it waits for the answer. */
export type Next = { kind: "open"; scope: DocScope; name: string } | { kind: "new"; scope: DocScope; found: string[] } | { kind: "project"; path: string } | { kind: "close" };

/** The sheet before copies change; `kind` tells Review from a delete or one copy's fix. */
export type Review = { title: string; preview: ConfigPreview; busy: boolean; conflict: boolean };

export type State = {
  project: string | null;
  view: LibraryView | null;
  open: OpenDoc | null;
  draft: Draft | null;
  errors: FormErrors;
  busy: boolean;
  /** The bar's line; a `snapshot` is what Undo puts back. */
  status: { text: string; snapshot?: string } | null;
  asking: Next | null;
  conflict: string | null;
  closed: boolean;
  review: Review | null;
  /** Counts the times the draft's text was replaced from outside the editor; the editor's key, so it remounts on the new text. */
  textRev: number;
};

export type Action =
  /** A view of the library, read for `project`; one read for another project than the page's is dropped. */
  | { type: "viewLoaded"; view: LibraryView; project: string | null }
  | { type: "open"; scope: DocScope; name: string }
  | { type: "newDoc"; scope: DocScope }
  | { type: "pickProject"; path: string }
  | { type: "closeAsked" }
  | { type: "discard" }
  | { type: "cancel" }
  | { type: "edit"; draft: Draft }
  | { type: "saveStarted" }
  | { type: "saved"; view: LibraryView; name: string }
  | { type: "saveRefused"; message: string }
  | { type: "saveConflict" }
  | { type: "reloaded"; view: LibraryView; project: string | null }
  | { type: "reviewShown"; title: string; preview: ConfigPreview }
  | { type: "reviewBusy" }
  | { type: "reviewConflict" }
  | { type: "reviewCancelled" }
  | { type: "applied"; snapshot: string }
  | { type: "undone" };

/** The first project is the one holding the folder, when it is a known project; the picker changes it. */
export function initialState(projects: ProjectInfo[], folder: string | null): State {
  const { selected } = pickProject(projects, folder ?? undefined);
  const known = selected !== null && projects.some((p) => p.path === selected);
  return { project: known ? selected : null, view: null, open: null, draft: null, errors: {}, busy: false, status: null, asking: null, conflict: null, closed: false, review: null, textRev: 0 };
}

export const conflictLine = (name: string) => `${name} changed on disk since you opened it.`;

/** The review sheet's title for each preview the page shows. */
export type Sheet = { kind: "review" } | { kind: "delete"; name: string } | { kind: "copy"; takeCopy: boolean };
export function sheetTitle(sheet: Sheet): string {
  if (sheet.kind === "review") return "Update the copies";
  if (sheet.kind === "delete") return `Delete ${sheet.name} and its copies`;
  return sheet.takeCopy ? "Use this copy's text" : "Put the doc's text back";
}

export const findDoc = (view: LibraryView | null, open: OpenDoc | null): LibraryDoc | null => {
  if (view === null || open === null) return null;
  const docs = open.scope === "user" ? view.user_docs : view.project_docs;
  return docs.find((d) => d.doc.name.toLowerCase() === open.name.toLowerCase())?.doc ?? null;
};

export const isDirtyNow = (s: State): boolean => s.draft !== null && isDirty(s.draft, findDoc(s.view, s.open));
/** Whether Save has anything to send: a new draft always (the core says what it lacks), an open doc only once edited. */
export const saveNeeded = (s: State): boolean => s.open === null || isDirtyNow(s);

/** The field a refused save's sentence belongs under, by what it says; any other sentence sits by the buttons. */
export function fieldOf(message: string): keyof FormErrors {
  if (/^(A doc needs a name|A name |Another doc|A file named )/.test(message)) return "name";
  if (/^A description/.test(message)) return "description";
  if (/\bglob/i.test(message)) return "globs";
  return "save";
}

const clear = { errors: {}, conflict: null, busy: false } as const;

function proceed(s: State, next: Next): State {
  switch (next.kind) {
    case "open": {
      const d = findDoc(s.view, { scope: next.scope, name: next.name });
      return { ...s, ...clear, asking: null, open: { scope: next.scope, name: d?.name ?? next.name }, draft: d === null ? null : draftOf(d) };
    }
    case "new":
      return { ...s, ...clear, asking: null, open: null, draft: newDraft(next.scope, next.found) };
    case "project":
      return { ...s, ...clear, asking: null, project: next.path, view: null, open: null, draft: null, status: null };
    case "close":
      return { ...s, asking: null, closed: true };
  }
}

const ask = (s: State, next: Next): State => (isDirtyNow(s) ? { ...s, asking: next } : proceed(s, next));

function step(s: State, a: Action): State {
  switch (a.type) {
    case "viewLoaded": {
      // An answer for a project the page has left (or not yet reached) would show another project's docs under this one.
      if (a.project !== s.project) return s;
      // A draft with no edits follows the fresh doc; one with edits is left alone.
      const fresh = findDoc(a.view, s.open);
      const follow = s.draft !== null && fresh !== null && !isDirtyNow(s);
      // The open doc is gone from the fresh view (deleted here or outside): nothing is open any more.
      if (s.open !== null && fresh === null) return { ...s, view: a.view, open: null, draft: null };
      return { ...s, view: a.view, draft: follow ? draftOf(fresh) : s.draft };
    }
    case "open":
      return ask(s, { kind: "open", scope: a.scope, name: a.name });
    case "newDoc":
      return ask(s, { kind: "new", scope: a.scope, found: s.view?.found ?? [] });
    case "pickProject":
      return ask(s, { kind: "project", path: a.path });
    case "closeAsked":
      return ask(s, { kind: "close" });
    case "discard":
      return s.asking === null ? s : proceed(s, s.asking);
    case "cancel":
      return { ...s, asking: null };
    case "edit":
      return { ...s, draft: a.draft };
    case "saveStarted":
      return { ...s, busy: true, errors: {}, conflict: null };
    case "saved": {
      // A saved doc is found under the name the draft had when it was saved; the page's view is the core's, after the save.
      const open: OpenDoc = { scope: s.draft?.scope ?? "user", name: a.name };
      const d = findDoc(a.view, open);
      return { ...s, view: a.view, open, draft: d === null ? s.draft : draftOf(d), busy: false, errors: {}, conflict: null, status: { text: `Saved ${a.name}.` } };
    }
    case "saveRefused":
      return { ...s, busy: false, errors: { [fieldOf(a.message)]: a.message } };
    case "saveConflict":
      return { ...s, busy: false, conflict: conflictLine(s.draft?.name ?? s.open?.name ?? "The doc") };
    case "reloaded": {
      if (a.project !== s.project) return s;
      const d = findDoc(a.view, s.open);
      return { ...s, view: a.view, draft: d === null ? s.draft : draftOf(d), conflict: null, errors: {} };
    }
    case "reviewShown":
      return { ...s, review: { title: a.title, preview: a.preview, busy: false, conflict: false } };
    case "reviewBusy":
      return s.review === null ? s : { ...s, review: { ...s.review, busy: true } };
    case "reviewConflict":
      return s.review === null ? s : { ...s, review: { ...s.review, busy: false, conflict: true } };
    case "reviewCancelled":
      return { ...s, review: null };
    case "applied":
      return { ...s, review: null, status: { text: "Copies updated.", snapshot: a.snapshot } };
    case "undone":
      return { ...s, status: { text: "Put back." } };
  }
}

/** An edit comes from the form (the editor's own typing too) and never remounts the editor; any other action that changes the draft's text does. */
export function instructionsReducer(s: State, a: Action): State {
  const n = step(s, a);
  if (a.type === "edit" || n.draft === null || n.draft.body === s.draft?.body) return n;
  return { ...n, textRev: s.textRev + 1 };
}

/** Ctrl+S saves only with no sheet or unsaved-edits question open over the page. */
export const saveKeyActs = (s: State): boolean => s.review === null && s.asking === null;
