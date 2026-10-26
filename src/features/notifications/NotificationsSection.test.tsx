import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import type { NotifySettings } from "../../generated/NotifySettings";
import { NotificationsView } from "./NotificationsSection";

const DEFAULTS: NotifySettings = {
  needs_you: { banner: true, sound: true },
  done: { banner: true, sound: true },
  error: { banner: true, sound: true },
  subagent_sound: false,
  quiet_hours: { enabled: false, start: "22:00", end: "08:00" },
  terminal_alerts: true,
};
const none = () => {};
const view = (settings: NotifySettings | null = DEFAULTS, error: string | null = null) =>
  renderToStaticMarkup(<NotificationsView settings={settings} error={error} onChange={none} />);
const group = (h: string, name: string) => h.match(new RegExp(`<section[^>]*aria-label="${name}"[^>]*>[\\s\\S]*?</section>`))?.[0] ?? "";
const switchLabels = (h: string) => [...h.matchAll(/<span class="switch__text">([^<]*)</g)].map((m) => m[1]);

describe("NotificationsView", () => {
  it("draws no dialog shell, title or Close button: the Settings page owns them", () => {
    const h = view();
    expect(h).not.toContain('role="dialog"');
    expect(h).not.toContain("<h1");
    expect(h).not.toContain("<h2");
    expect(h).not.toContain(">Close</button>");
    expect(h).not.toContain("backdrop");
  });
  it("starts its subheadings at h3, under the page's section heading", () => {
    expect(view()).toContain('<h3 class="notifications-dialog__name">Needs you</h3>');
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
  it("renders the terminal alerts switch, on by default", () => {
    const h = view();
    const g = group(h, "Terminal programs");
    expect(switchLabels(g)).toEqual(["Alerts from terminal programs (bell, OSC 9, OSC 777)"]);
    expect(g).toContain('<h3 class="notifications-dialog__name">Terminal programs</h3>');
    expect(g).toMatch(/<input[^>]*role="switch"[^>]*checked=""/);
    expect(g).toContain("Off: the row still changes, with no banner or sound.");
    expect(h.indexOf('aria-label="Terminal programs"')).toBeGreaterThan(h.indexOf('aria-label="Subagent done"'));
    expect(h.indexOf('aria-label="Terminal programs"')).toBeLessThan(h.indexOf('aria-label="Quiet hours"'));
    expect(group(view({ ...DEFAULTS, terminal_alerts: false }), "Terminal programs")).not.toContain('checked=""');
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
    const withChoice = (closeToTray: boolean | null) => renderToStaticMarkup(<NotificationsView settings={DEFAULTS} closeToTray={closeToTray} error={null} onChange={none} />);
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
  it("shows the notification centre switch only when the link state is known", () => {
    const withLink = (link?: boolean | null) =>
      renderToStaticMarkup(<NotificationsView settings={DEFAULTS} error={null} link={link} onChange={none} />);
    const h = withLink(false);
    expect(h).toContain("Notification centre");
    expect(h).toContain("Open sessions from the notification centre");
    expect(h).toContain("Adds a marshell:// link for your Windows account. It can only bring Marshell forward and select a session.");
    expect(h.indexOf("Notification centre")).toBeGreaterThan(h.indexOf('aria-label="Terminal programs"'));
    expect(h.indexOf("Notification centre")).toBeLessThan(h.indexOf('aria-label="Close button"'));
    expect(group(withLink(true), "Notification centre")).toMatch(/<input[^>]*role="switch"[^>]*checked=""/);
    for (const none of [null, undefined]) expect(withLink(none)).not.toContain("Notification centre");
  });
});
