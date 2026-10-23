import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";

const read = (path: string) => readFileSync(new URL(path, import.meta.url), "utf8").replace(/\/\*[\s\S]*?\*\//g, "");
const zIndexOf = (css: string, selector: string) => {
  const rule = new RegExp(`(?:^|\\})\\s*${selector.replace(/[.[\]"=-]/g, "\\$&")}\\s*\\{([^}]*)\\}`).exec(css);
  expect(rule, selector).not.toBeNull();
  const body = rule?.[1];
  if (body === undefined) throw new Error(`Missing rule: ${selector}`);
  const z = /z-index:\s*(\d+)/.exec(body);
  return z ? Number(z[1]) : 0;
};

// Each of these covers the whole live window as a sibling of the sidebar, so it has to sit above the sidebar's resize handle.
const DIALOGS: [string, string][] = [
  ["../departures/LiveDepartures.css", ".departures"],
  ["../appearance/AppearanceDialog.css", ".appearance-dialog"],
  ["../sessions/LiveShortcuts.css", ".live-shortcuts"],
  ["../settings/ProfilesPage.css", ".profiles-dialog"],
  ["../tasks/ReviewSheet.css", ".review"],
  ["../notifications/NotificationsDialog.css", ".notifications-dialog"],
  ["../agents/AgentsDialog.css", ".agents-dialog"],
  ["../diagnostics/DiagnosticsDialog.css", ".diagnostics-dialog"],
  ["./CloseDialog.css", ".close-dialog"],
];

describe("Full-window dialogs stack above the window's own controls", () => {
  const handle = zIndexOf(read("./MainWindow.css"), ".window__side-handle");
  it.each(DIALOGS)("%s keeps the sidebar's resize handle under it, so the handle can't be dragged through the dialog", (file, selector) => {
    expect(zIndexOf(read(file), selector)).toBeGreaterThan(handle);
  });
});
