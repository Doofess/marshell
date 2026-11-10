import codes from "../../../src-tauri/src/browser/codes.json";
import { sessionShortcut } from "../../lib/appShortcuts";
import type { KeyEventLike } from "../../lib/bindings";
import type { Os } from "../../lib/keymap";
import type { FocusKind } from "../../lib/navigation/keys";
import { isPerfToggle } from "../perf/perfToggle";
import { claimsKey } from "../sessions/useNavigation";
import type { Chord } from "./browserApi";

const COMBOS = [false, true].flatMap((ctrl) => [false, true].flatMap((shift) => [false, true].map((alt) => ({ ctrl, shift, alt }))));

/**
 * The chords the app takes from a page: every code the shell knows (`codes.json`, which Rust's `vk_to_code` is tested against)
 * with each of the 8 Ctrl, Shift and Alt combinations, kept when `claims` takes it. Esc and Ctrl+L are the shell's own rule.
 */
export function claimedChords(claims: (e: KeyEventLike) => boolean): Chord[] {
  const out: Chord[] = [];
  for (const code of codes) {
    for (const { ctrl, shift, alt } of COMBOS) {
      if (claims({ code, ctrlKey: ctrl, shiftKey: shift, altKey: alt, metaKey: false })) out.push({ code, ctrl, shift, alt });
    }
  }
  return out;
}

/** What a terminal's key handler gives up to the app (TerminalView): the perf toggle, a session shortcut, a navigation chord. */
export function appClaims(os: Os, focus: FocusKind): (e: KeyEventLike) => boolean {
  return (e) => isPerfToggle({ ...e, type: "keydown" }) || sessionShortcut(e, os) !== null || claimsKey(e, os, focus);
}
