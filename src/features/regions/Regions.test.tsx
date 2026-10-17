import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it, vi } from "vitest";
import { RegionComment, popoverKeyDown } from "./RegionComment";
import { RegionLayer } from "./RegionLayer";
import { describeRegion } from "./region";

const noop = () => {};
const comment = (more: Partial<Parameters<typeof RegionComment>[0]> = {}) =>
  renderToStaticMarkup(<RegionComment target={{ name: "billing", via: "paste" }} busy={false} error={null} onAdd={noop} onCancel={noop} {...more} />);

describe("RegionComment", () => {
  it("says where the region goes", () => {
    expect(comment()).toContain("Adds to billing");
    expect(comment({ target: { name: "billing", via: "clipboard" } })).toContain("Copies to the clipboard");
  });
  it("says why a paused or ended session takes nothing, in place of the target, with Add disabled", () => {
    const out = comment({ target: { name: "migration", refused: "migration is paused." } });
    expect(out).toContain("migration is paused.");
    expect(out).not.toContain("Adds to");
    expect(out).toContain('aria-disabled="true"');
  });
  it("has a labelled input and the Add and Cancel buttons", () => {
    const out = comment();
    expect(out).toContain("Comment");
    expect(out).toContain('placeholder="What should the agent look at?"');
    expect(out).toContain(">Add<");
    expect(out).toContain(">Cancel<");
  });
  it("disables Add with no session", () => {
    const out = comment({ target: null });
    expect(out).toContain("No session to add it to.");
    expect(out).toContain('aria-disabled="true"');
  });
  it("shows an error in an alert", () => {
    expect(comment({ error: "It is paused." })).toMatch(/role="alert"[^>]*>It is paused\./);
  });
});

describe("RegionComment while Add is in flight", () => {
  it("keeps the input focusable: read-only, not disabled, and Add says it is disabled", () => {
    const out = comment({ busy: true });
    const input = out.match(/<input[^>]*>/)![0];
    expect(input).not.toMatch(/\sdisabled/);
    expect(input).toMatch(/readOnly|readonly/);
    expect(out).toContain('aria-disabled="true"');
  });
  it("is editable otherwise", () => {
    expect(comment().match(/<input[^>]*>/)![0]).not.toMatch(/readOnly|readonly/);
  });
});

describe("popoverKeyDown", () => {
  const key = (k: string) => ({ key: k, preventDefault: vi.fn(), stopPropagation: vi.fn() });
  it("Esc goes back to the box from anywhere in the popover", () => {
    const onCancel = vi.fn();
    const e = key("Escape");
    popoverKeyDown(e, onCancel);
    expect(onCancel).toHaveBeenCalledTimes(1);
    expect(e.preventDefault).toHaveBeenCalled();
    expect(e.stopPropagation).toHaveBeenCalled();
  });
  it("other keys do not cancel, and none reach the pane", () => {
    const onCancel = vi.fn();
    const e = key("a");
    popoverKeyDown(e, onCancel);
    expect(onCancel).not.toHaveBeenCalled();
    expect(e.stopPropagation).toHaveBeenCalled();
  });
});

describe("RegionLayer", () => {
  it("is a described region box", () => {
    const r = { page: 2, x: 0.1, y: 0.2, w: 0.4, h: 0.25 };
    const out = renderToStaticMarkup(<RegionLayer region={r} onChange={noop} onConfirm={noop} onCancel={noop} />);
    expect(out).toContain('aria-roledescription="region"');
    expect(out).toContain(`aria-label="${describeRegion(r)}"`);
  });
});
