import "./SelectedMark.css";

/** Which ARIA state tells assistive technology that the item is selected: options use aria-selected, items of a list of places aria-current. */
export type SelectedKind = "selected" | "current";

/**
 * The props that make an element show and announce its selection the shared way: the `is-selected-mark` class (styled by
 * SelectedMark.css) plus the matching ARIA state, set only while it is true.
 */
export function selectedProps(selected: boolean, kind: SelectedKind = "selected", className = ""): { className: string; "aria-selected"?: boolean; "aria-current"?: "true" } {
  const cls = `${className} is-selected-mark`.trim();
  if (kind === "current") return selected ? { className: cls, "aria-current": "true" } : { className: cls };
  return { className: cls, "aria-selected": selected };
}
