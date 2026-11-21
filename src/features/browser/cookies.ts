import type { MenuItem } from "../../components/ContextMenu/menuKeys";
import type { BrowserSettings } from "../../generated/BrowserSettings";
import type { CookieRule } from "../../generated/CookieRule";

/** `label` is the menu row's, `said` the rule in a sentence. */
export const COOKIE_ROWS: readonly { rule: CookieRule; label: string; said: string }[] = [
  { rule: "always", label: "Keep always", said: "Always" },
  { rule: "until-quit", label: "Keep until quit", said: "Until quit" },
  { rule: "never", label: "Never keep", said: "Never" },
];

export const SETTINGS_UNREAD = "Your settings couldn't be read";
const NOT_YET = "Not available on this system yet";

/** The three radio rows. With `now` null (the settings aren't read) none is checked and each is off. */
export function cookieRows(now: CookieRule | null, pages: boolean): MenuItem[] {
  return COOKIE_ROWS.map(({ rule, label }): MenuItem => {
    const disabled = !pages ? NOT_YET : now === null ? SETTINGS_UNREAD : undefined;
    return { id: `cookies:${rule}`, label, radio: true, checked: now === rule, ...(disabled === undefined ? {} : { disabled }) };
  });
}

export function cookieChoice(id: string): CookieRule | undefined {
  return COOKIE_ROWS.find((r) => id === `cookies:${r.rule}`)?.rule;
}

/** The rule to tell the shell: the saved one, or "never" when the settings can't be read; null while they are still loading. */
export function ruleToTell(settings: BrowserSettings | null, failed: boolean): CookieRule | null {
  if (settings !== null) return settings.cookies;
  return failed ? "never" : null;
}

/** Whether a change of rule needs the loaded pages made again: a page's privacy is decided when it is made, and only "never" is private. */
export function needsReload(from: CookieRule | null, to: CookieRule): boolean {
  return from !== null && (from === "never") !== (to === "never");
}

export function cookiesSaid(rule: CookieRule, reloaded: number): string {
  const said = COOKIE_ROWS.find((r) => r.rule === rule)?.said ?? rule;
  if (reloaded === 0) return `Cookies: ${said}.`;
  return `Cookies: ${said}. ${reloaded} ${reloaded === 1 ? "tab was" : "tabs were"} loaded again.`;
}
