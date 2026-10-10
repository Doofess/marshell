/** A rectangle in window pixels. The navigation layer only reads where things are; it never touches the DOM. */
export type Rect = { x: number; y: number; w: number; h: number };

/** Something the navigation cursor can rest on. */
export type Target =
  /** A terminal on screen, by its slot (0 to 3). The slot may be empty. */
  | { kind: "pane"; index: number }
  /** A session in the sidebar or the rail, by its session number. */
  | { kind: "row"; number: number }
  /** A control in the top bar, or the launcher and palette buttons: it is activated, not selected. */
  | { kind: "top"; id: string };

export type Dir = "left" | "right" | "up" | "down";

/** Where each target is right now, measured by the window and handed in. */
export type NavLayout = Array<{ target: Target; rect: Rect }>;

/** What the window shows: the session number in each pane slot (null = empty slot), where everything is, and names for announcements. */
export type NavContext = {
  panes: Array<number | null>;
  layout: NavLayout;
  names?: Record<number, string>;
};

export type NavState = {
  /** The slot holding the terminal that has the keyboard, or null: nothing is selected. */
  selectedPane: number | null;
  /** Where the navigation cursor is while the modifier is held and the user is moving about; it selects on release. */
  cursor: Target | null;
  /** The navigation modifier is down. */
  held: boolean;
  /** A Tab or an arrow was used during this hold, so further arrows are navigation (before that, in a text box, they select text). */
  engaged: boolean;
};

export type NavEvent =
  | { type: "modifier-down" }
  | { type: "modifier-up" }
  /** The navigation chord's Tab. It cycles the terminals on screen and never reaches the sidebar. */
  | { type: "tab" }
  | { type: "arrow"; dir: Dir }
  /** The chord that selects no terminal. */
  | { type: "select-none" }
  /** Esc: cancels the cursor while one is out; otherwise deselects, but the adapter only sends it when focus is not in a terminal. */
  | { type: "escape" }
  /** A click or tap. */
  | { type: "choose"; target: Target };

export type NavEffect =
  | { type: "select-pane"; index: number }
  /** A session that is not on screen was chosen: show it in this slot, then select that slot. */
  | { type: "bring-into-view"; number: number; pane: number }
  | { type: "activate"; id: string }
  | { type: "deselect" }
  | { type: "announce"; text: string };

export const INITIAL_NAV: NavState = { selectedPane: null, cursor: null, held: false, engaged: false };

export const sameTarget = (a: Target | null, b: Target | null): boolean => {
  if (!a || !b || a.kind !== b.kind) return false;
  if (a.kind === "pane") return a.index === (b as typeof a).index;
  if (a.kind === "row") return a.number === (b as typeof a).number;
  return a.id === (b as typeof a).id;
};
