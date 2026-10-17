import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { RegionComment } from "./RegionComment";
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

describe("RegionLayer", () => {
  it("is a described region box", () => {
    const r = { page: 2, x: 0.1, y: 0.2, w: 0.4, h: 0.25 };
    const out = renderToStaticMarkup(<RegionLayer region={r} onChange={noop} onConfirm={noop} onCancel={noop} />);
    expect(out).toContain('aria-roledescription="region"');
    expect(out).toContain(`aria-label="${describeRegion(r)}"`);
  });
});
