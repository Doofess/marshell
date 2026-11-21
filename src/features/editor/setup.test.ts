import { insertBracket } from "@codemirror/autocomplete";
import { indentMore } from "@codemirror/commands";
import { gotoLine } from "@codemirror/search";
import { EditorSelection, EditorState } from "@codemirror/state";
import { keymap, type KeyBinding } from "@codemirror/view";
import { describe, expect, it } from "vitest";
import { findCommand } from "../../lib/bindings";
import { COMMANDS, shortcutLabel, type Os } from "../../lib/keymap";
import { EDITOR_DEFAULTS } from "./editorPrefs";
import { editorChords, editorKeys, extensions, indentOf, optionsOn, prefsEffects } from "./setup";

const make = (o: Partial<Parameters<typeof extensions>[0]> = {}, doc = "one\ntwo") =>
  EditorState.create({ doc, extensions: extensions({ readOnly: false, wrap: false, name: "a.txt", appChord: () => false, prefs: EDITOR_DEFAULTS, ...o }) });

/** Every key a state's keymaps name, in any platform's field. */
const named = (state: EditorState): string[] =>
  state
    .facet(keymap)
    .flat()
    .flatMap((b: KeyBinding) => [b.key, b.mac, b.win, b.linux, b.shift && b.key ? `Shift-${b.key}` : undefined])
    .filter((k): k is string => k !== undefined);

/** A key by its modifiers and its base key, so "Shift-Mod-k" and "Mod-Shift-k" are the same. */
const canon = (name: string) => {
  const parts = name.split(/-(?!$)/);
  const key = parts.pop()!.toLowerCase();
  return [...parts.map((p) => p.toLowerCase()).sort(), key].join("-");
};

const OS: Os[] = ["windows", "mac", "linux"];

describe("setup", () => {
  it("the_extensions_build_a_state", () => {
    const state = make();
    expect(state.doc.toString()).toBe("one\ntwo");
    expect(state.facet(EditorState.tabSize)).toBe(4);
    expect(state.facet(EditorState.allowMultipleSelections)).toBe(false);
  });

  it("no_app_shortcut_is_bound_in_the_editor", () => {
    const labels = (os: Os) => COMMANDS.filter((c) => c.binding).map((c) => shortcutLabel(c.binding!, os));
    for (const os of OS) {
      const keys = editorKeys(os);
      expect(keys.length, os).toBeGreaterThan(20);
      for (const label of labels(os)) expect(keys, `${os} ${label}`).not.toContain(label);
      // The chords themselves, which is what the window reads: no editor key is a key press the app takes.
      for (const chord of editorChords(os)) {
        const hit = findCommand({ code: chord.code, ctrlKey: chord.ctrl, shiftKey: chord.shift, altKey: chord.alt, metaKey: chord.meta }, os);
        expect(hit?.id, `${os} ${JSON.stringify(chord)}`).toBeUndefined();
      }
    }
    // The ones that clash on Windows are the ones the brief names, so the check is not vacuous there.
    expect(editorKeys("windows")).not.toContain("Ctrl+Shift+K");
    expect(editorKeys("windows")).toContain("Ctrl+F");
  });

  it("the_left_out_keys_are_not_bound", () => {
    const bound = new Set(named(make()).map(canon));
    for (const key of ["Mod-d", "Mod-Shift-l", "Mod-Shift-k", "Alt-ArrowLeft", "Alt-ArrowRight", "Mod-Alt-g"]) expect(bound.has(canon(key)), key).toBe(false);
    // No command adds a cursor either.
    expect(bound.has(canon("Mod-Alt-ArrowUp"))).toBe(false);
    expect(bound.has(canon("Mod-Alt-ArrowDown"))).toBe(false);
  });

  it("a_key_the_app_takes_is_left_out", () => {
    const state = make({ appChord: (e) => e.code === "KeyF" && e.ctrlKey });
    expect(new Set(named(state).map(canon)).has("mod-f")).toBe(false);
    expect(new Set(named(make()).map(canon)).has("mod-f")).toBe(true);
  });

  it("ctrl_g_and_alt_z_are_bound", () => {
    const bindings = make().facet(keymap).flat();
    expect(bindings.find((b) => b.key === "Mod-g")?.run).toBe(gotoLine);
    expect(bindings.some((b) => b.key === "Alt-z" && typeof b.run === "function")).toBe(true);
  });

  it("esc_always_makes_the_next_tab_leave", () => {
    // CodeMirror arms this itself only when no binding took the Esc, and one does whenever text is selected.
    const first = make().facet(keymap).flat().find((b) => b.key === "Escape");
    const armed: number[] = [];
    const view = { setTabFocusMode: (ms: number) => armed.push(ms) } as unknown as Parameters<NonNullable<KeyBinding["run"]>>[0];
    expect(first?.run?.(view)).toBe(false);
    expect(armed).toEqual([2000]);
  });

  it("a_bracket_gets_its_closing_one", () => {
    const tr = insertBracket(make({}, ""), "(");
    expect(tr?.state.doc.toString()).toBe("()");
  });

  it("one_cursor_only", () => {
    const state = make();
    const next = state.update({ selection: EditorSelection.create([EditorSelection.cursor(0), EditorSelection.cursor(5)]) }).state;
    expect(next.selection.ranges).toHaveLength(1);
  });

  it("read_only_refuses_a_change", () => {
    const state = make({ readOnly: true });
    expect(state.readOnly).toBe(true);
    let dispatched = 0;
    expect(indentMore({ state, dispatch: () => dispatched++ })).toBe(false);
    expect(dispatched).toBe(0);
    expect(indentMore({ state: make(), dispatch: () => dispatched++ })).toBe(true);
    expect(dispatched).toBe(1);
  });

  it("the_phrases_are_the_plans", () => {
    const state = make();
    const want: Record<string, string> = {
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
    for (const [key, text] of Object.entries(want)) expect(state.phrase(key), key).toBe(text);
  });

  it("the_indent_follows_the_file", () => {
    expect(indentOf("a\n\tb\n    c")).toBe("\t");
    expect(indentOf("a\n  b\n    c")).toBe("  ");
    expect(indentOf("a\n    b")).toBe("    ");
    // A comment's one space is not an indent, and a file with none gets four spaces, the tab size.
    expect(indentOf("/**\n * x\n */\nfoo()")).toBe("    ");
    expect(indentOf("")).toBe("    ");
    // A deeper first line is cut to eight.
    expect(indentOf("a\n            b")).toBe("        ");
    // A Makefile's recipes need a tab, and Go is written with tabs, whatever the file holds so far.
    expect(indentOf("", "make")).toBe("\t");
    expect(indentOf("x:\n    y", "make")).toBe("\t");
    expect(indentOf("", "go")).toBe("\t");
  });

  it("a file whose indent can't be told takes the tab size in spaces", () => {
    expect(indentOf("", undefined, 2)).toBe("  ");
    expect(indentOf("", undefined, 8)).toBe("        ");
    // A file with an indent of its own keeps it, and a language that needs a tab keeps that.
    expect(indentOf("a\n    b", undefined, 2)).toBe("    ");
    expect(indentOf("a\n  b", undefined, 8)).toBe("  ");
    expect(indentOf("", "go", 2)).toBe("\t");
  });
});

describe("the Files settings in the editor", () => {
  const prefs = { ...EDITOR_DEFAULTS };

  it("tab size sets how wide a tab is drawn", () => {
    expect(make({ prefs: { ...prefs, tab_size: 2 } }).facet(EditorState.tabSize)).toBe(2);
    expect(make({ prefs: { ...prefs, tab_size: 8 } }).facet(EditorState.tabSize)).toBe(8);
  });

  it("Tab key indents off leaves Tab unbound", () => {
    const keys = (tab_indents: boolean) => named(make({ prefs: { ...prefs, tab_indents } }));
    expect(keys(true)).toContain("Tab");
    expect(keys(false)).not.toContain("Tab");
    // Shift-Tab goes with it, and the rest of the keymap stays.
    expect(keys(false).map(canon)).not.toContain(canon("Shift-Tab"));
    expect(keys(false)).toContain("Alt-z");
  });

  it("indent guides, whitespace and scroll past end are on only when asked", () => {
    expect(optionsOn(make({ prefs: { ...prefs, indent_guides: false, whitespace: false, scroll_past_end: false } })).sort()).toEqual([]);
    expect(optionsOn(make({ prefs: { ...prefs, indent_guides: true, whitespace: false, scroll_past_end: false } })).sort()).toEqual(["guides"]);
    expect(optionsOn(make({ prefs: { ...prefs, indent_guides: false, whitespace: true, scroll_past_end: false } })).sort()).toEqual(["whitespace"]);
    expect(optionsOn(make({ prefs: { ...prefs, indent_guides: false, whitespace: false, scroll_past_end: true } })).sort()).toEqual(["scrollPastEnd"]);
    expect(optionsOn(make({ prefs: { ...prefs, indent_guides: true, whitespace: true, scroll_past_end: true } })).sort()).toEqual(["guides", "scrollPastEnd", "whitespace"]);
  });

  it("reconfigurePrefs applies a change to a live view without rebuilding it", () => {
    let state = make({ prefs: { ...prefs, tab_size: 4, tab_indents: true, indent_guides: false } });
    // A view needs a DOM; the effects are what reconfigurePrefs sends, so a state applies them the same way.
    const effects = prefsEffects({ ...prefs, tab_size: 2, tab_indents: false, indent_guides: true, whitespace: true }, () => false);
    state = state.update({ effects }).state;
    expect(state.facet(EditorState.tabSize)).toBe(2);
    expect(named(state)).not.toContain("Tab");
    expect(optionsOn(state).sort()).toEqual(["guides", "whitespace"]);
  });
});
