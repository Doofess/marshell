import { closeBrackets, closeBracketsKeymap } from "@codemirror/autocomplete";
import { defaultKeymap, history, historyKeymap, indentWithTab } from "@codemirror/commands";
import { bracketMatching, indentOnInput } from "@codemirror/language";
import { findPrevious, gotoLine, search, searchKeymap } from "@codemirror/search";
import { Compartment, EditorState, Facet, type Extension, type StateEffect } from "@codemirror/state";
import { drawSelection, EditorView, highlightActiveLine, highlightWhitespace, keymap, lineNumbers, scrollPastEnd, type Command, type KeyBinding } from "@codemirror/view";
import { findCommand, type Chord } from "../../lib/bindings";
import { keyText, type Os } from "../../lib/keymap";
import { APP_KEYS_BOX } from "../tasks/reviewOpen";
import type { EditorPrefs } from "./editorPrefs";
import { indentGuides } from "./indentGuides";

/** Word wrap: on or off, switched without rebuilding the editor. */
export const WRAP: Compartment = new Compartment();
/** Read-only and the label that says so. */
export const READ_ONLY: Compartment = new Compartment();

/** How wide a tab is drawn. */
const TAB_SIZE: Compartment = new Compartment();
/** The keymap, rebuilt when Tab stops or starts indenting. */
const KEYS: Compartment = new Compartment();
const GUIDES: Compartment = new Compartment();
const WHITESPACE: Compartment = new Compartment();
const PAST_END: Compartment = new Compartment();
/** What the Tab key puts in. The editor owns it: the file's own indent decides it, and the tab size where the file has none. */
export const INDENT: Compartment = new Compartment();

/** Which of the optional drawings are on: for a test, since CodeMirror's own extensions can't be told apart in a state. */
const optionOn = Facet.define<string, string[]>({ combine: (v) => [...v] });
export const optionsOn = (state: EditorState): string[] => state.facet(optionOn);

const guides = (on: boolean): Extension => (on ? [indentGuides(), optionOn.of("guides")] : []);
const whitespace = (on: boolean): Extension => (on ? [highlightWhitespace(), optionOn.of("whitespace")] : []);
const pastEnd = (on: boolean): Extension => (on ? [scrollPastEnd(), optionOn.of("scrollPastEnd")] : []);
const keys = (prefs: EditorPrefs, appChord: (e: KeyboardEvent) => boolean): Extension => keymap.of(bindings(here(), appChord, prefs.tab_indents));

/** The words CodeMirror's find bar and go-to-line box use, as the plan spells them. */
const PHRASES: Record<string, string> = {
  Find: "Find",
  Replace: "Replace",
  replace: "Replace",
  next: "Next",
  previous: "Previous",
  all: "All",
  "match case": "Match case",
  regexp: "Regular expression",
  "by word": "Whole word",
  "replace all": "Replace all",
  close: "Close",
  "Go to line": "Go to line",
  go: "Go",
};

/** Whether word wrap is on, which CodeMirror's own extension does not tell. */
const wrapOn = Facet.define<boolean, boolean>({ combine: (v) => v.some(Boolean) });

/** Word wrap as an extension. */
export const wrapping = (on: boolean): Extension => (on ? [EditorView.lineWrapping, wrapOn.of(true)] : []);

/** Read-only, and the name a screen reader hears: "{name}, editor" or "{name}, read-only". */
export const access = (name: string, readOnly: boolean): Extension => [EditorState.readOnly.of(readOnly), EditorView.contentAttributes.of({ "aria-label": `${name}, ${readOnly ? "read-only" : "editor"}` })];

/** Switches word wrap. Shared by Alt+Z and the palette's "Word wrap". */
export const toggleWrap: Command = (view) => {
  const on = view.state.facet(wrapOn);
  view.dispatch({ effects: WRAP.reconfigure(wrapping(!on)) });
  return true;
};

/** Languages whose files are indented with a tab whatever they hold: a Makefile's recipes need one, and Go is written with them. */
const TAB_LANGUAGES = ["make", "go"];

/**
 * What the Tab key puts in: the file's own indent, read from its first indented line among the first 500. A tab stays a tab,
 * spaces stay as many spaces (two to eight), and a file with no indent yet gets the tab size in spaces (four unless the Files
 * settings say otherwise).
 */
export function indentOf(text: string, language?: string, tabSize = 4): string {
  if (language !== undefined && TAB_LANGUAGES.includes(language)) return "\t";
  let from = 0;
  for (let n = 0; n < 500 && from <= text.length; n++) {
    const end = text.indexOf("\n", from);
    const line = text.slice(from, end === -1 ? text.length : end);
    if (line.startsWith("\t")) return "\t";
    const spaces = line.length - line.trimStart().length;
    // One space is a comment's continuation, not an indent, and a blank line tells nothing.
    if (spaces >= 2 && line.trim() !== "" && line.startsWith(" ".repeat(spaces))) return " ".repeat(Math.min(spaces, 8));
    if (end === -1) break;
    from = end + 1;
  }
  return " ".repeat(tabSize);
}

/* Keys. A key is a CodeMirror key name ("Mod-Shift-k"). Mod is Ctrl, and Cmd on macOS. */

/** Keys of CodeMirror's own keymaps that the editor leaves out: they clash with the app's or the pane's keys, or they add a second cursor. */
const LEFT_OUT = ["Mod-d", "Mod-Shift-l", "Mod-Shift-k", "Alt-ArrowLeft", "Alt-ArrowRight", "Mod-Alt-g", "Mod-Alt-ArrowUp", "Mod-Alt-ArrowDown"];

type Platform = "key" | "mac" | "win" | "linux";
type Modifiers = { ctrl: boolean; shift: boolean; alt: boolean; meta: boolean };
type Parsed = Modifiers & { key: string; mod: boolean };

/** The platform CodeMirror reads its keymaps for. The same test it uses, so what the editor leaves out is what it would have bound. */
const here = (): Platform => (typeof navigator === "undefined" ? "key" : /Mac/.test(navigator.platform) ? "mac" : /Win/.test(navigator.platform) ? "win" : /Linux|X11/.test(navigator.platform) ? "linux" : "key");

function parse(name: string): Parsed {
  const parts = name.split(/-(?!$)/);
  let key = parts.pop()!;
  if (key === "Space") key = " ";
  const p: Parsed = { key, ctrl: false, shift: false, alt: false, meta: false, mod: false };
  for (const m of parts) {
    if (/^(cmd|meta|m)$/i.test(m)) p.meta = true;
    else if (/^(a|alt|option|opt)$/i.test(m)) p.alt = true;
    else if (/^(c|ctrl|control)$/i.test(m)) p.ctrl = true;
    else if (/^(s|shift)$/i.test(m)) p.shift = true;
    else if (/^mod$/i.test(m)) p.mod = true;
  }
  return p;
}

/** A key's modifiers and base key without the choice of platform, so "Shift-Mod-k" and "Mod-Shift-k" are the same. */
function same(a: string, b: string): boolean {
  const x = parse(a);
  const y = parse(b);
  const flags = (p: Parsed) => `${p.ctrl}${p.shift}${p.alt}${p.meta}${p.mod}`;
  return flags(x) === flags(y) && x.key.toLowerCase() === y.key.toLowerCase();
}

const CODES: Record<string, string> = { "[": "BracketLeft", "]": "BracketRight", "\\": "Backslash", "/": "Slash", "-": "Minus", "=": "Equal", ",": "Comma", ".": "Period", ";": "Semicolon", "'": "Quote", "`": "Backquote", " ": "Space" };

/** The physical key and modifiers a key name stands for on a platform. An upper-case letter is Shift and the letter. */
export function chordOfKey(name: string, platform: Platform): Chord {
  const p = parse(name);
  const mac = platform === "mac";
  let shift = p.shift;
  let key = p.key;
  if (key.length === 1 && key !== key.toLowerCase()) {
    shift = true;
    key = key.toLowerCase();
  }
  const code = /^[a-z]$/.test(key) ? `Key${key.toUpperCase()}` : /^[0-9]$/.test(key) ? `Digit${key}` : (CODES[key] ?? key);
  return { code, ctrl: p.ctrl || (p.mod && !mac), shift, alt: p.alt, meta: p.meta || (p.mod && mac) };
}

/**
 * Esc makes the next Tab leave the editor, for two seconds. CodeMirror does this itself only when no binding took the Esc, and
 * one does whenever text is selected (it drops the selection), so the Tab after it would indent instead. This runs first and
 * takes nothing: the other Esc bindings still run.
 */
const armTabFocus = (view: EditorView): boolean => {
  view.setTabFocusMode(2000);
  return false;
};

/** A key press for a chord, as far as the app's shortcut matching reads one. */
function eventOf(c: Chord): KeyboardEvent {
  return { code: c.code, key: c.code.replace(/^(Key|Digit)/, "").toLowerCase(), ctrlKey: c.ctrl, shiftKey: c.shift, altKey: c.alt, metaKey: c.meta, repeat: false, isComposing: false, getModifierState: () => false } as unknown as KeyboardEvent;
}

/** What the editor binds, on a platform: CodeMirror's keymaps, less what is left out and less every key `appChord` says the app takes. */
function bindings(platform: Platform, appChord: (e: KeyboardEvent) => boolean, tabIndents = true): KeyBinding[] {
  const wrap: KeyBinding = { key: "Alt-z", run: toggleWrap, scope: "editor search-panel", preventDefault: true };
  const line: KeyBinding = { key: "Mod-g", run: gotoLine, scope: "editor search-panel", preventDefault: true };
  // Mod-g is go to line, so find next and previous keep F3 and Shift-F3 (and Shift-Mod-g for the previous one).
  const finds = searchKeymap.filter((b) => b.key !== "Mod-g");
  const sources: readonly KeyBinding[] = [{ key: "Escape", run: armTabFocus }, ...closeBracketsKeymap, line, wrap, ...finds, { key: "Shift-Mod-g", run: findPrevious, scope: "editor search-panel", preventDefault: true }, ...historyKeymap, ...(tabIndents ? [indentWithTab] : []), ...defaultKeymap];
  const taken = (key: string) => appChord(eventOf(chordOfKey(key, platform)));
  const out: KeyBinding[] = [];
  for (const b of sources) {
    if (b.key && LEFT_OUT.some((k) => same(k, b.key!))) continue;
    // CodeMirror picks the platform's key itself; the binding is flattened to that one, so the check below is on the key in use.
    const key = (platform === "key" ? undefined : b[platform]) ?? b.key;
    if (key === undefined) continue;
    const run = b.run && !taken(key) ? b.run : undefined;
    const shift = b.shift && !taken(`Shift-${key}`) ? b.shift : undefined;
    if (!run && !shift) continue;
    const { key: _key, mac: _mac, win: _win, linux: _linux, run: _run, shift: _shift, ...rest } = b;
    out.push({ ...rest, key, ...(run ? { run } : {}), ...(shift ? { shift } : {}) });
  }
  return out;
}

const PLATFORM: Record<Os, Platform> = { windows: "win", mac: "mac", linux: "linux" };

/** The chords the editor binds on a system, once the app's own are left out: for the test that no app shortcut is among them. */
export function editorChords(os: Os): Chord[] {
  const platform = PLATFORM[os];
  const app = (e: KeyboardEvent) => findCommand(e, os) !== undefined;
  return bindings(platform, app).flatMap((b) => [...(b.run ? [chordOfKey(b.key!, platform)] : []), ...(b.shift ? [chordOfKey(`Shift-${b.key}`, platform)] : [])]);
}

function label(c: Chord, os: Os): string {
  const key = keyText(c.code);
  if (os === "mac") return `${c.ctrl ? "⌃" : ""}${c.meta ? "⌘" : ""}${c.alt ? "⌥" : ""}${c.shift ? "⇧" : ""}${key}`;
  return [c.ctrl && "Ctrl", c.alt && "Alt", c.shift && "Shift", c.meta && "Meta", key].filter(Boolean).join("+");
}

/** The keys the editor's keymap binds on a system, spelled as the app spells a shortcut. */
export function editorKeys(os: Os): string[] {
  return [...new Set(editorChords(os).map((c) => label(c, os)))];
}

/**
 * The editor's extensions. `appChord` answers whether a key press is one of the app's shortcuts; a binding on such a key is
 * left out, so the key reaches the window's own handler. Tab indents; Esc, then Tab, leaves (CodeMirror's tab-focus mode, which
 * Esc turns on for two seconds), and Ctrl+M switches Tab between the two. Esc closes the find bar and nothing more.
 */
export function extensions(o: { readOnly: boolean; wrap: boolean; name: string; appChord: (e: KeyboardEvent) => boolean; prefs: EditorPrefs }): Extension[] {
  return [
    lineNumbers(),
    history(),
    drawSelection(),
    highlightActiveLine(),
    bracketMatching(),
    closeBrackets(),
    indentOnInput(),
    search({ top: true }),
    EditorState.phrases.of(PHRASES),
    EditorState.allowMultipleSelections.of(false),
    TAB_SIZE.of(EditorState.tabSize.of(o.prefs.tab_size)),
    EditorView.contentAttributes.of({ role: "textbox", "aria-multiline": "true", spellcheck: "false", class: APP_KEYS_BOX }),
    READ_ONLY.of(access(o.name, o.readOnly)),
    WRAP.of(wrapping(o.wrap)),
    GUIDES.of(guides(o.prefs.indent_guides)),
    WHITESPACE.of(whitespace(o.prefs.whitespace)),
    PAST_END.of(pastEnd(o.prefs.scroll_past_end)),
    KEYS.of(keys(o.prefs, o.appChord)),
  ];
}

/**
 * What a change of the Files settings reconfigures, for a view that is open: how wide a tab is drawn, whether Tab indents (the
 * keymap is rebuilt), the guides, the whitespace marks and the room past the last line. Word wrap is the tab's (`WRAP`).
 */
export function prefsEffects(prefs: EditorPrefs, appChord: (e: KeyboardEvent) => boolean): StateEffect<unknown>[] {
  return [
    TAB_SIZE.reconfigure(EditorState.tabSize.of(prefs.tab_size)),
    GUIDES.reconfigure(guides(prefs.indent_guides)),
    WHITESPACE.reconfigure(whitespace(prefs.whitespace)),
    PAST_END.reconfigure(pastEnd(prefs.scroll_past_end)),
    KEYS.reconfigure(keys(prefs, appChord)),
  ];
}

/** Applies a change of the Files settings to an open view. */
export function reconfigurePrefs(view: EditorView, prefs: EditorPrefs, appChord: (e: KeyboardEvent) => boolean): void {
  view.dispatch({ effects: prefsEffects(prefs, appChord) });
}
