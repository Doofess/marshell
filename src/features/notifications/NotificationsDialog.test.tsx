import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import type { NotifySettings } from "../../generated/NotifySettings";
import { NotificationsDialogView } from "./NotificationsDialog";

const DEFAULTS: NotifySettings = {
  needs_you: { banner: true, sound: true },
  done: { banner: true, sound: true },
  error: { banner: true, sound: true },
  subagent_sound: false,
  quiet_hours: { enabled: false, start: "22:00", end: "08:00" },
};
const none = () => {};
const view = (settings: NotifySettings | null = DEFAULTS, error: string | null = null) =>
  renderToStaticMarkup(<NotificationsDialogView settings={settings} error={error} onChange={none} onClose={none} />);
const group = (h: string, name: string) => h.match(new RegExp(`<section[^>]*aria-label="${name}"[^>]*>[\\s\\S]*?</section>`))?.[0] ?? "";
const switchLabels = (h: string) => [...h.matchAll(/<span class="switch__text">([^<]*)</g)].map((m) => m[1]);

describe("NotificationsDialogView", () => {
  it("is a modal dialog named Notifications", () => {
    expect(view()).toMatch(/role="dialog"[^>]*aria-modal="true"[^>]*aria-label="Notifications"/);
  });
  it("gives Needs you, Done and Error a Banner and a Sound switch each", () => {
    const h = view();
    for (const name of ["Needs you", "Done", "Error"]) expect(switchLabels(group(h, name))).toEqual(["Banner", "Sound"]);
  });
  it("gives Subagent done one Sound switch, off by default, with its help", () => {
    const g = group(view(), "Subagent done");
    expect(switchLabels(g)).toEqual(["Sound"]);
    expect(g).not.toContain('checked=""');
    expect(g).toContain("A short sound when a subagent finishes.");
  });
  it("shows each switch in its saved state", () => {
    const h = view({ ...DEFAULTS, done: { banner: false, sound: true } });
    expect(group(h, "Done").match(/checked=""/g)).toHaveLength(1);
  });
  it("offers Quiet hours with an on switch, two time fields prefilled 22:00 and 08:00, and its help", () => {
    const g = group(view(), "Quiet hours");
    expect(switchLabels(g)).toEqual(["Quiet hours"]);
    expect(g).toMatch(/<input[^>]*type="time"[^>]*value="22:00"/);
    expect(g).toMatch(/<input[^>]*type="time"[^>]*value="08:00"/);
    expect(g).toContain("No sounds or taskbar flashes between these times. Banners still arrive, silently.");
  });
  it("labels every time field", () => {
    const g = group(view(), "Quiet hours");
    expect(g).toMatch(/<label[^>]*>(?:(?!<\/label>)[\s\S])*From(?:(?!<\/label>)[\s\S])*<input/);
    expect(g).toMatch(/<label[^>]*>(?:(?!<\/label>)[\s\S])*To(?:(?!<\/label>)[\s\S])*<input/);
  });
  it("has a Close button group with one checkbox for keeping Marshell in the tray, checked unless the choice is no", () => {
    const withChoice = (closeToTray: boolean | null) => renderToStaticMarkup(<NotificationsDialogView settings={DEFAULTS} closeToTray={closeToTray} error={null} onChange={none} onClose={none} />);
    for (const [choice, checked] of [[null, true], [true, true], [false, false]] as const) {
      const g = group(withChoice(choice), "Close button");
      expect(switchLabels(g)).toEqual(["Keep Marshell running in the tray when I close the window"]);
      expect(g).toContain('type="checkbox"');
      expect(g.includes('checked=""')).toBe(checked);
    }
    expect(group(view(null), "Close button")).toBe("");
  });
  it("makes no promise about Windows do-not-disturb", () => {
    expect(view().toLowerCase()).not.toMatch(/do.not.disturb/);
  });
  it("shows a reading line until the settings are read, and no switch", () => {
    const h = view(null);
    expect(h).toContain("Reading your settings…");
    expect(h).not.toContain('role="switch"');
  });
  it("shows a save error as an alert", () => {
    expect(view(DEFAULTS, "Save failed.")).toMatch(/role="alert"[^>]*>Save failed\./);
  });
});
