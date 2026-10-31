import { COMPANY_NAMES, shownSections, type CompanyAgent, type SettingsSection } from "./settingsSections";

/**
 * The palette commands that open the Settings page, and the section each opens at: the old per-page commands keep their labels
 * and land on their section, and Settings… opens Appearance. Anything else is not a Settings command.
 */
export function paletteSection(id: string): SettingsSection | null {
  switch (id) {
    case "profiles":
    case "appearance":
    case "agents":
    case "notifications":
      return id;
    case "diagnostics":
      return "advanced";
    case "edit-references":
      return "projects";
    case "history-settings":
      return "history";
    case "settings":
      return "appearance";
    default:
      return null;
  }
}

/** Where the tray's Settings… opens: the section shown last in this run when it is still listed, else Appearance. */
export function settingsStart(last: SettingsSection | null, shown: readonly SettingsSection[]): SettingsSection {
  return last !== null && shown.includes(last) ? last : "appearance";
}

/**
 * `settingsStart` for the tray's Settings…, when the last section may belong to a CLI uninstalled since: before reopening at a CLI's
 * section, the CLIs are read again (`found` is that read, for the caller to keep). Any other section opens without a read, and a
 * failed read keeps what was known.
 */
export async function freshSettingsStart(
  last: SettingsSection | null,
  known: ReadonlySet<CompanyAgent>,
  readFound: () => Promise<ReadonlySet<CompanyAgent>>,
): Promise<{ start: SettingsSection; found: ReadonlySet<CompanyAgent> | null }> {
  if (last === null || !(last in COMPANY_NAMES)) return { start: settingsStart(last, shownSections(known).map((s) => s.id)), found: null };
  const found = await readFound().catch(() => null);
  return { start: settingsStart(last, shownSections(found ?? known).map((s) => s.id)), found };
}

/** A jump to a session closes the page, so the selection never changes behind it: Ctrl+Shift+1 to 9, the oldest waiting, a palette session. */
export function closesSettings(command: string): boolean {
  return command === "next-waiting" || /^focus-session-[1-9]$/.test(command) || /^session:./.test(command);
}

/** A section mounts the first time it is shown. The same list comes back when it already is, so a render sets no state. */
export function mountedWith(mounted: SettingsSection[], section: SettingsSection): SettingsSection[] {
  return mounted.includes(section) ? mounted : [...mounted, section];
}

type Focusable = { focus(): void };
type HeadingRoot = { querySelector(selector: string): Focusable | null };

/** The shown section's heading (`h2`, `tabIndex={-1}`): where focus lands on open and on a section change, and where stray focus is pulled back to. */
export function settingsFocusFallback(root: HeadingRoot | null): Focusable | null {
  return root?.querySelector(".settings-shell__heading") ?? null;
}

/** On open and on a section change, focus goes to the shown section's heading, not to a control. */
export function focusSettingsHeading(root: HeadingRoot | null): void {
  settingsFocusFallback(root)?.focus();
}

const PRECEDING = 2;
const FOLLOWING = 4;

/**
 * Where Tab (or Shift+Tab) goes on the page: the index in `controls`, or null when there are none. From a control it is the next
 * or previous one, wrapping. From an element that isn't one of them (the section heading, `tabIndex={-1}`) it is the first
 * control after it in document order, or for Shift+Tab the last one before it, wrapping when there is none. With no focus it is
 * the first, or the last.
 */
export function settingsTabTarget<T extends { compareDocumentPosition(other: T): number }>(controls: readonly T[], active: T | null, shift: boolean): number | null {
  const count = controls.length;
  if (count === 0) return null;
  const at = active ? controls.indexOf(active) : -1;
  if (at >= 0) return shift ? (at === 0 ? count - 1 : at - 1) : at === count - 1 ? 0 : at + 1;
  if (!active) return shift ? count - 1 : 0;
  if (shift) {
    for (let i = count - 1; i >= 0; i--) if (active.compareDocumentPosition(controls[i]!) & PRECEDING) return i;
    return count - 1;
  }
  const next = controls.findIndex((c) => active.compareDocumentPosition(c) & FOLLOWING);
  return next < 0 ? 0 : next;
}

const TEXT_TYPES = new Set(["text", "search", "number", "email", "url", "tel", "password", "date", "time", "datetime-local", "month", "week"]);

/**
 * A field where typing is the point: a textarea, a contenteditable, or an input that takes text, a number or a date. A radio, a
 * switch (a checkbox), a range, a colour, a file input and the buttons are not, so a shortcut still works while one has the focus.
 * xterm's hidden textarea is the terminal, not a field. Duck-typed, so it needs no DOM to test.
 */
export function isTextEntry(el: { tagName?: string; type?: string; isContentEditable?: boolean; classList?: { contains(name: string): boolean } } | null): boolean {
  if (!el) return false;
  if (el.isContentEditable === true) return true;
  if (el.tagName === "TEXTAREA") return !el.classList?.contains("xterm-helper-textarea");
  return el.tagName === "INPUT" && TEXT_TYPES.has((el.type ?? "text").toLowerCase());
}

type Control = { closest(selector: string): unknown; tabIndex?: number; type?: string; name?: string; checked?: boolean };

/**
 * The controls Tab walks: a section kept mounted after a switch is hidden, and its controls are out of reach. A group of native
 * radios is one stop, at its checked radio (else its first), as the browser's own Tab has it; the arrows move inside the group.
 */
export function visibleControls<T extends Control>(controls: T[]): T[] {
  const shown = controls.filter((c) => !c.closest("[hidden]") && (c.tabIndex ?? 0) >= 0);
  const radios = (name: string) => shown.filter((c) => c.type === "radio" && c.name === name);
  return shown.filter((c) => {
    if (c.type !== "radio" || !c.name) return true;
    const group = radios(c.name);
    return c.checked || (!group.some((r) => r.checked) && group[0] === c);
  });
}

/** Closing is refused while the shell profiles hold unsaved edits, as their dialog refused: the change bar stays until saved or discarded. */
export function closeOrRefuse(profilesCanClose: boolean): "close" | "refuse" {
  return profilesCanClose ? "close" : "refuse";
}
