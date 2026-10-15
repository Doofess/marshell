import { COMMANDS, type Binding, type Command, type Os } from "./keymap";
import { DEFAULT_NAV_MODIFIER, type NavModifier } from "./navigation/modifier";

/** The part of a KeyboardEvent shortcut matching reads. Matching uses `code`, the physical key, never `key`, the character it types. */
export type KeyEventLike = { code: string; ctrlKey: boolean; shiftKey: boolean; altKey: boolean; metaKey: boolean };
export type Chord = { code: string; ctrl: boolean; shift: boolean; alt: boolean; meta: boolean };

const NAV_KEYS: Record<NavModifier, Pick<Chord, "ctrl" | "shift" | "alt">> = {
  "ctrl+shift": { ctrl: true, shift: true, alt: false },
  "ctrl+alt": { ctrl: true, shift: false, alt: true },
  "alt+shift": { ctrl: false, shift: true, alt: true },
};

/** The exact keys a binding needs: Ctrl+Shift (Cmd on macOS) plus Alt when asked, the key alone, Shift (plus Alt) without the app modifier, or the navigation modifier. */
export function chordOf(b: Binding, os: Os, nav: NavModifier = DEFAULT_NAV_MODIFIER): Chord {
  if (b.plain) return { code: b.code, ctrl: false, shift: false, alt: false, meta: false };
  if (b.shift) return { code: b.code, ctrl: false, shift: true, alt: !!b.alt, meta: false };
  if (b.nav) return { code: b.code, ...NAV_KEYS[nav], meta: false };
  if (os === "mac") return { code: b.code, ctrl: false, shift: false, alt: !!b.alt, meta: true };
  return { code: b.code, ctrl: true, shift: true, alt: !!b.alt, meta: false };
}

const same = (a: Chord, b: Chord) => a.code === b.code && a.ctrl === b.ctrl && a.shift === b.shift && a.alt === b.alt && a.meta === b.meta;

export function matchesBinding(e: KeyEventLike, b: Binding, os: Os, nav: NavModifier = DEFAULT_NAV_MODIFIER): boolean {
  return same({ code: e.code, ctrl: e.ctrlKey, shift: e.shiftKey, alt: e.altKey, meta: e.metaKey }, chordOf(b, os, nav));
}

/** The command a key press belongs to, or undefined, in which case the key goes to the terminal untouched. */
export function findCommand(e: KeyEventLike, os: Os, nav: NavModifier = DEFAULT_NAV_MODIFIER, commands: Command[] = COMMANDS): Command | undefined {
  return commands.find((c) => c.binding && matchesBinding(e, c.binding, os, nav));
}

export function bindingConflicts(a: Binding, b: Binding, os: Os, nav: NavModifier = DEFAULT_NAV_MODIFIER): boolean {
  return same(chordOf(a, os, nav), chordOf(b, os, nav));
}

type Reserved = { os: Os | "all"; chord: Partial<Chord> & { code: string }; reason: string };
const ctrlShift = { ctrl: true, shift: true, alt: false, meta: false } as const;
const cmd = { ctrl: false, shift: false, alt: false, meta: true } as const;
const bare = { ctrl: false, shift: false, alt: false, meta: false } as const;

/** Chords an app binding must not take: the system's, a text box's, or a terminal program's. Each carries the reason that is shown when a user tries to rebind to it. */
export const RESERVED: Reserved[] = [
  // macOS: the standard app menu and editing keys, which the system or a focused text box takes first.
  { os: "mac", chord: { code: "KeyM", ...cmd }, reason: "⌘M is Minimize." },
  { os: "mac", chord: { code: "KeyH", ...cmd }, reason: "⌘H is Hide." },
  { os: "mac", chord: { code: "KeyQ", ...cmd }, reason: "⌘Q is Quit." },
  { os: "mac", chord: { code: "KeyZ", ...cmd }, reason: "⌘Z is Undo in a text box." },
  { os: "mac", chord: { code: "KeyX", ...cmd }, reason: "⌘X is Cut in a text box." },
  { os: "mac", chord: { code: "KeyC", ...cmd }, reason: "⌘C is Copy." },
  { os: "mac", chord: { code: "KeyV", ...cmd }, reason: "⌘V is Paste." },
  { os: "mac", chord: { code: "KeyA", ...cmd }, reason: "⌘A is Select all in a text box." },
  { os: "mac", chord: { code: "Tab", ...cmd }, reason: "⌘Tab is the system app switcher, so no app ever sees it." },
  { os: "mac", chord: { code: "Space", ...cmd }, reason: "⌘Space is Spotlight." },
  { os: "mac", chord: { code: "Escape", ...cmd, alt: true }, reason: "⌘⌥Esc is Force Quit." },
  // Windows and Linux.
  { os: "windows", chord: { code: "Escape", ...ctrlShift }, reason: "Ctrl+Shift+Esc opens Task Manager, so no app ever sees it." },
  { os: "windows", chord: { code: "KeyZ", ...ctrlShift }, reason: "Ctrl+Shift+Z is Redo in a text box." },
  { os: "linux", chord: { code: "KeyZ", ...ctrlShift }, reason: "Ctrl+Shift+Z is Redo in a text box." },
  { os: "all", chord: { code: "KeyC", ...ctrlShift }, reason: "Ctrl+Shift+C is how a terminal copies; terminals never use plain Ctrl+C for that." },
  { os: "all", chord: { code: "KeyV", ...ctrlShift }, reason: "Ctrl+Shift+V is how a terminal pastes." },
  { os: "windows", chord: { code: "KeyI", ...ctrlShift }, reason: "Ctrl+Shift+I opens the webview's developer tools in a development build." },
  { os: "linux", chord: { code: "KeyI", ...ctrlShift }, reason: "Ctrl+Shift+I opens the webview's developer tools in a development build." },
  { os: "windows", chord: { code: "KeyJ", ...ctrlShift }, reason: "Ctrl+Shift+J opens the webview's developer console in a development build." },
  { os: "linux", chord: { code: "KeyJ", ...ctrlShift }, reason: "Ctrl+Shift+J opens the webview's developer console in a development build." },
  { os: "linux", chord: { code: "KeyU", ...ctrlShift }, reason: "Ctrl+Shift+U starts Unicode entry in GTK and IBus." },
  { os: "linux", chord: { code: "KeyE", ...ctrlShift }, reason: "Ctrl+Shift+E opens the emoji picker in IBus." },
  { os: "linux", chord: { code: "KeyT", ctrl: true, shift: false, alt: true, meta: false }, reason: "Ctrl+Alt+T opens a terminal on Ubuntu." },
  { os: "windows", chord: { code: "ArrowLeft", ...ctrlShift }, reason: "Ctrl+Shift+Left selects a word in a text box." },
  { os: "windows", chord: { code: "ArrowRight", ...ctrlShift }, reason: "Ctrl+Shift+Right selects a word in a text box." },
  { os: "linux", chord: { code: "ArrowLeft", ...ctrlShift }, reason: "Ctrl+Shift+Left selects a word in a text box." },
  { os: "linux", chord: { code: "ArrowRight", ...ctrlShift }, reason: "Ctrl+Shift+Right selects a word in a text box." },
  // A bare key belongs to the program in the terminal. (F-keys are not listed: programs rarely need F2, and F2 is the one the plan keeps.)
  ...["Tab", "Escape", "Enter", "Backspace", "Space", "ArrowUp", "ArrowDown", "ArrowLeft", "ArrowRight"].map((code): Reserved => ({ os: "all", chord: { code, ...bare }, reason: `A bare ${code} goes to the program in the terminal (completion, interrupt, menus), so it cannot be a shortcut.` })),
];

const EVERYTHING = { ctrl: false, shift: false, alt: false, meta: false };

/** Why this binding cannot be used on this system, or null when nobody else wants it. */
export function reservedReason(b: Binding, os: Os, nav: NavModifier = DEFAULT_NAV_MODIFIER): string | null {
  const c = chordOf(b, os, nav);
  const hit = RESERVED.find((r) => (r.os === "all" || r.os === os) && same(c, { ...EVERYTHING, ...r.chord }));
  return hit ? hit.reason : null;
}

/** Every duplicate and every reserved binding among the commands on this system, as sentences. Empty when the keymap is clean. */
export function problemsIn(commands: Command[], os: Os, nav: NavModifier = DEFAULT_NAV_MODIFIER): string[] {
  const out: string[] = [];
  const bound = commands.filter((c): c is Command & { binding: Binding } => !!c.binding);
  bound.forEach((c, i) => {
    for (const d of bound.slice(i + 1)) if (bindingConflicts(c.binding, d.binding, os, nav)) out.push(`${c.id} and ${d.id} share a binding.`);
    const why = reservedReason(c.binding, os, nav);
    if (why) out.push(`${c.id}: ${why}`);
  });
  return out;
}
