/** The selected session's terminal takes the keyboard when it appears or its session is selected, unless an overlay blocks it. */
export const focusOnChange = (s: { selected: boolean; blocked: boolean }): boolean => s.selected && !s.blocked;

/** An overlay closing hands the keyboard to the selected terminal only when it restored focus to nothing; focus it put back on what opened it stays there. */
export const focusOnUnblock = (s: { selected: boolean; onNothing: boolean }): boolean => s.selected && s.onNothing;
