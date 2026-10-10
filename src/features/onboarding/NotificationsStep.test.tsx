import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { nameContainsLabel } from "../../lib/labelInName";
import { NotificationsStep } from "./NotificationsStep";

describe("NotificationsStep", () => {
  it("previews every sound with a button named after it", () => {
    const h = renderToStaticMarkup(<NotificationsStep permission="not-asked" os="windows" />);
    for (const s of ["Needs you", "Escalation", "Done", "Subagent done", "Error"]) expect(h).toContain(`Play ${s} sound`);
    expect(h).toContain("Ask Windows to allow notifications");
  });
  it("says what still works when notifications are blocked, and where to turn them on", () => {
    const h = renderToStaticMarkup(<NotificationsStep permission="denied" os="windows" />);
    expect(h).toContain("Windows is blocking notifications for Marshell");
    expect(h).toContain("Settings");
    expect(h).toContain("the badge and the lane still work");
  });
  it("is quiet once allowed and says banners never raise the window", () => {
    const h = renderToStaticMarkup(<NotificationsStep permission="granted" os="windows" />);
    expect(h).toContain("Allowed");
    expect(h).toContain("A banner never takes focus from what you are typing in");
  });
  it("has no live region and no exclamation mark", () => {
    const h = renderToStaticMarkup(<NotificationsStep permission="granted" os="windows" />);
    expect(h).not.toMatch(/aria-live|role="status"|role="alert"|!/);
  });
  it("names its buttons from the text they show (the play triangle is decoration)", () => {
    const h = renderToStaticMarkup(<NotificationsStep permission="not-asked" os="windows" />);
    for (const b of h.match(/<button[\s\S]*?<\/button>/g) ?? []) expect(nameContainsLabel(b)).toBe(true);
  });
  it("swaps the OS word for macOS and Linux", () => {
    expect(renderToStaticMarkup(<NotificationsStep permission="not-asked" os="mac" />)).toContain("Ask macOS to allow notifications");
    expect(renderToStaticMarkup(<NotificationsStep permission="not-asked" os="linux" />)).toContain("Ask your desktop to allow notifications");
  });
});
