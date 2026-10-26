import { useEffect, useId, useRef, useState, type KeyboardEvent } from "react";
import { BUNDLED_FONT, BUNDLED_FONTS, familyOf, fontChoiceOf, fontStack } from "./terminalFont";
import "../../components/a11y/SelectedMark.css";
import "./FontPicker.css";

export const SAMPLE_LINE = "0O 1lI {} => ~/src $ git status";

type Key = "match" | (typeof BUNDLED_FONTS)[number] | "other";
type PickerState = { other: boolean; text: string };

/** Picking "Another font…": the field opens, holding the saved text when that is what is saved. Nothing is saved yet. */
export function chooseOther(state: PickerState, family: string): PickerState {
  const choice = fontChoiceOf(family);
  return { other: true, text: choice.kind === "other" ? choice.family : state.text };
}

/** Committing the field. A name is picked; an empty field picks nothing and the choice shown before comes back. */
export function commitText(state: PickerState, family: string): { state: PickerState; pick: string | null } {
  const name = state.text.trim();
  if (name) return { state: { other: false, text: "" }, pick: familyOf(fontChoiceOf(name)) };
  const choice = fontChoiceOf(family);
  return { state: { other: false, text: choice.kind === "other" ? choice.family : "" }, pick: null };
}

/** Esc puts the saved text back and is handled here, so a popup around the picker stays open. */
export function fieldKey(key: string, draft: string, saved: string): { next: string; prevent: boolean } {
  return key === "Escape" ? { next: saved, prevent: true } : { next: draft, prevent: false };
}

const OPTIONS: readonly (readonly [Key, string])[] = [
  ["match", "Match my terminal"],
  ["JetBrains Mono", "JetBrains Mono"],
  ["Cascadia Mono", "Cascadia Mono"],
  ["Atkinson Hyperlegible Mono", "Atkinson Hyperlegible Mono"],
  ["other", "Another font…"],
];

/**
 * The terminal font as a list of native radios, each name drawn in its own font above a sample line. The parent saves the
 * string `onPick` gives and says "Terminal font: <label>".
 */
export function FontPicker({ family, wtFamily, onPick }: { family: string; wtFamily?: string; onPick: (family: string) => void }) {
  const fieldId = useId();
  const name = useId();
  const field = useRef<HTMLInputElement>(null);
  const choice = fontChoiceOf(family);
  const saved = choice.kind === "other" ? choice.family : "";
  const [state, setState] = useState<PickerState>({ other: false, text: saved });
  const [focusField, setFocusField] = useState(false);
  const showOther = state.other || choice.kind === "other";
  const current: Key = showOther ? "other" : choice.kind === "match" ? "match" : choice.family;

  useEffect(() => {
    if (focusField && showOther) {
      field.current?.focus();
      setFocusField(false);
    }
  }, [focusField, showOther]);

  const stackOf = (key: Key) => {
    const resolved = key === "match" ? (wtFamily?.trim() || BUNDLED_FONT) : key === "other" ? (state.other ? state.text.trim() : saved) || BUNDLED_FONT : key;
    return fontStack(resolved, wtFamily);
  };

  const choose = (key: Key) => {
    if (key === "other") {
      if (!showOther) setState(chooseOther(state, family));
      setFocusField(true);
      return;
    }
    setState({ other: false, text: saved });
    const next = familyOf(key === "match" ? { kind: "match" } : { kind: "bundled", family: key });
    if (next !== familyOf(choice)) onPick(next);
  };

  const commit = () => {
    const out = commitText(state, family);
    setState(out.state);
    if (out.pick !== null && out.pick !== familyOf(choice)) onPick(out.pick);
  };

  const onKeyDown = (e: KeyboardEvent<HTMLInputElement>) => {
    if (e.key === "Enter") return commit();
    const { next, prevent } = fieldKey(e.key, state.text, saved);
    if (prevent) {
      e.preventDefault();
      setState((s) => ({ ...s, text: next }));
    }
  };

  const text = state.other || choice.kind !== "other" ? state.text : saved;

  return (
    <fieldset className="font-picker">
      <legend className="field__label">Terminal font</legend>
      <div className="font-picker__list">
        {OPTIONS.map(([key, label]) => (
          <label key={key} className="font-picker__option is-selected-mark" data-selected={key === current ? "true" : undefined} style={{ fontFamily: stackOf(key) }}>
            <input type="radio" name={name} value={key} checked={key === current} onChange={() => choose(key)} />
            <span className="font-picker__name">{label}</span>
            <span className="font-picker__sample" aria-hidden="true">
              {SAMPLE_LINE}
            </span>
          </label>
        ))}
      </div>
      {showOther && (
        <div className="font-picker__other">
          <label className="field__label" htmlFor={fieldId}>
            Font family
          </label>
          <input ref={field} id={fieldId} type="text" className="field__control" data-field="font" spellCheck={false} value={text} onChange={(e) => setState({ other: showOther, text: e.target.value })} onBlur={commit} onKeyDown={onKeyDown} />
        </div>
      )}
    </fieldset>
  );
}
