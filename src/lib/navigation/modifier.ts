import type { Os } from "../keymap";

/**
 * The keys held down for the navigation layer: with them down, Tab and the arrows move the selection and nothing reaches the
 * terminal program. Plain Tab, plain Esc and plain arrows always go to the program (shell completion, agent suggestions,
 * Claude Code's interrupt, vim, fzf), which is why navigation needs a chord of its own.
 */
export type NavModifier = "ctrl+shift" | "ctrl+alt" | "alt+shift";

/** Ctrl+Shift everywhere: Windows, Linux and macOS give it no job of its own (Ctrl+Shift alone only changes the input language when released without another key). */
export const DEFAULT_NAV_MODIFIER: NavModifier = "ctrl+shift";

type Flags = { ctrlKey: boolean; shiftKey: boolean; altKey: boolean; metaKey: boolean };
type Choice = { id: NavModifier; keys: { ctrl: boolean; shift: boolean; alt: boolean }; warn: Partial<Record<Os, string>> };

export const NAV_MODIFIERS: Choice[] = [
  { id: "ctrl+shift", keys: { ctrl: true, shift: true, alt: false }, warn: {} },
  {
    id: "ctrl+alt",
    keys: { ctrl: true, shift: false, alt: true },
    warn: {
      windows: "Windows reads Ctrl+Alt as AltGr, which types characters such as @ { [ on many layouts, so navigation would eat them.",
      mac: "Ctrl+Option is the VoiceOver key; with VoiceOver on, it is taken.",
      linux: "Ctrl+Alt+arrow changes workspace on GNOME and KDE, so the desktop takes it first.",
    },
  },
  {
    id: "alt+shift",
    keys: { ctrl: false, shift: true, alt: true },
    warn: {
      windows: "Alt+Shift switches the keyboard layout on Windows.",
      linux: "Alt+Shift switches the keyboard layout on most Linux desktops.",
      mac: "Option+Shift types symbols (Option+Shift+key), so navigation would take them.",
    },
  },
];

const byId = (m: NavModifier) => NAV_MODIFIERS.find((x) => x.id === m)!;

/** True when exactly the modifier's keys are down (and no others), so a three-key chord such as Ctrl+Shift+Alt+T never counts as navigation. */
export function isNavModifierHeld(f: Flags, m: NavModifier): boolean {
  const k = byId(m).keys;
  return f.ctrlKey === k.ctrl && f.shiftKey === k.shift && f.altKey === k.alt && !f.metaKey;
}

export function navModifierLabel(os: Os, m: NavModifier): string {
  const k = byId(m).keys;
  if (os === "mac") return `${k.ctrl ? "⌃" : ""}${k.alt ? "⌥" : ""}${k.shift ? "⇧" : ""}`;
  return [k.ctrl && "Ctrl", k.alt && "Alt", k.shift && "Shift"].filter(Boolean).join("+");
}

/** Why a choice is a bad idea on this system; empty means no known clash. The settings page shows these next to the choice. */
export function navModifierWarnings(os: Os, m: NavModifier): string[] {
  const w = byId(m).warn[os];
  return w ? [w] : [];
}
