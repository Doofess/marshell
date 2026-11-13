/**
 * The selected terminal's focus props. A link's hold only keeps it from taking the keyboard on its own: picking the terminal ends
 * the hold and turns autoFocus on, which focuses it. An overlay blocks it instead, so focus the overlay restores stays put.
 */
export const terminalFocusProps = (s: { selected: boolean; linkHeld: boolean; overlayUp: boolean }): { autoFocus: boolean; focusBlocked: boolean } => ({
  autoFocus: s.selected && !s.linkHeld,
  focusBlocked: s.overlayUp,
});

/** The selected session's terminal takes the keyboard when it appears or its session is selected, unless an overlay blocks it. */
export const focusOnChange = (s: { selected: boolean; blocked: boolean }): boolean => s.selected && !s.blocked;

/** An overlay closing hands the keyboard to the selected terminal only when it restored focus to nothing; focus it put back on what opened it stays there. */
export const focusOnUnblock = (s: { selected: boolean; onNothing: boolean }): boolean => s.selected && s.onNothing;
