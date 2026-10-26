import type { SettingsSection } from "./settingsSections";

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
    case "history-settings":
      return "history";
    case "settings":
      return "appearance";
    default:
      return null;
  }
}

/** Where the tray's Settings… opens: the section shown last in this run, else Appearance. */
export function settingsStart(last: SettingsSection | null): SettingsSection {
  return last ?? "appearance";
}

/** A jump to a session closes the page, so the selection never changes behind it: Ctrl+Shift+1 to 9, the oldest waiting, a palette session. */
export function closesSettings(command: string): boolean {
  return command === "next-waiting" || /^focus-session-[1-9]$/.test(command) || /^session:./.test(command);
}

/** A section mounts the first time it is shown. The same list comes back when it already is, so a render sets no state. */
export function mountedWith(mounted: SettingsSection[], section: SettingsSection): SettingsSection[] {
  return mounted.includes(section) ? mounted : [...mounted, section];
}

/** On open and on a section change, focus goes to the shown section's heading (`h2`, `tabIndex={-1}`), not to a control. */
export function focusSettingsHeading(root: { querySelector(selector: string): { focus(): void } | null } | null): void {
  root?.querySelector(".settings-shell__heading")?.focus();
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
