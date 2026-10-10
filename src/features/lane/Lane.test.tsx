import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { APPROVALS } from "../approval/fixtures";
import type { ApprovalRequest } from "../approval/types";
import { Lane } from "./Lane";
import { laneLayout } from "./laneLayout";

const html = (requests: ApprovalRequest[] = [APPROVALS.safeBash, APPROVALS.question], h = 800) => renderToStaticMarkup(<Lane requests={requests} sidebarHeight={h} />);

describe("Lane", () => {
  it("always renders its header, saying All clear when empty", () => {
    const h = html([]);
    expect(h).toContain("lane__header");
    expect(h).toContain("All clear");
    expect(h).not.toContain("approve-card");
  });
  it("expands only the oldest waiting request and shows the rest as one-liners", () => {
    const h = html();
    expect((h.match(/data-phase="pending"/g) ?? []).length).toBe(1);
    expect(h).toContain("Which AWS region");
    expect((h.match(/class="lane-row"/g) ?? []).length).toBe(1);
    expect(h.indexOf("Which AWS region")).toBeLessThan(h.indexOf("lane-row"));
  });
  it("tells a screen reader how many need you", () => {
    expect(html()).toContain('role="status"');
    expect(html()).toContain("Needs you · 2");
  });
  const many = Array.from({ length: 12 }, (_, i) => ({ ...APPROVALS.safeBash, id: `r${i}`, waitingMs: 1000 * (i + 1) }));
  it("shows +N more, as a button, only when it had to leave requests out", () => {
    expect(html()).not.toContain("more");
    const h = html(many, 800);
    expect(h).toMatch(/<button[^>]*class="lane__more"[^>]*aria-label="\+\d+ more, show all 12 requests"[^>]*>\+\d+ more<\/button>/);
    expect(h).toContain('data-clipped="true"');
  });
  it("renders only the requests that fit, so nothing hidden can take keyboard focus", () => {
    const h = html(many, 800);
    const shown = (h.match(/class="lane-row"/g) ?? []).length + (h.match(/data-phase="pending"/g) ?? []).length;
    expect(shown).toBe(laneLayout(12, 800).visible);
    expect(shown).toBeLessThan(12);
  });
  it("never offers a scroll: no scrolling attribute, and the count in the button matches what is left out", () => {
    const h = html(many, 400);
    expect(h).not.toContain("data-scrolls");
    expect(h).toContain(`+${laneLayout(12, 400).hiddenCount} more`);
  });
  it("ignores requests that are no longer pending", () => {
    expect(html([APPROVALS.receipt])).toContain("All clear");
  });
});

describe("Lane (accessibility gate)", () => {
  it("announces the count from a status wrapper, not by giving the heading a status role", () => {
    const h = html();
    expect(h).toMatch(/<div[^>]*role="status"[^>]*><h2 class="lane__heading">Needs you \u00b7 2<\/h2><\/div>/);
    expect(h).not.toMatch(/<h2[^>]*role=/);
  });
  it("names a one-liner with the text it shows, in order, then what the glyph says", () => {
    const h = html();
    expect(h).toContain('aria-label="billing Run a command 1m. Needs permission."');
    expect(h).toMatch(/lane-row__name"[^>]*>billing<\/span> <span class="lane-row__what"/);
  });
});
