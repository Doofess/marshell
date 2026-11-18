import { describe, expect, it } from "vitest";
import { pageRegionStep, type PageRegion } from "./pageRegion";

const URL = "https://example.com/docs";
const started = (): PageRegion => pageRegionStep(null, { type: "start", url: URL });
const box = (): PageRegion => pageRegionStep(started(), { type: "picture", ok: true });
const comment = (): PageRegion => pageRegionStep(box(), { type: "confirm" });

describe("pageRegionStep", () => {
  it("pageRegion_waits_for_the_picture_then_shows_the_box", () => {
    const s = started();
    expect(s).toMatchObject({ step: "wait", busy: false, error: null, url: URL });
    expect(pageRegionStep(s, { type: "picture", ok: true })).toMatchObject({ step: "box", url: URL });
    // The box starts where a PDF's does: 40% x 25%, centred.
    expect(s?.region).toEqual({ page: 1, x: 0.3, y: 0.375, w: 0.4, h: 0.25 });
  });
  it("a picture that comes after the box is up changes nothing", () => {
    const s = box();
    expect(pageRegionStep(s, { type: "picture", ok: true })).toBe(s);
  });
  it("pageRegion_ends_when_the_capture_failed", () => {
    expect(pageRegionStep(started(), { type: "picture", ok: false })).toBeNull();
  });
  it("pageRegion_ends_when_the_page_moved", () => {
    expect(pageRegionStep(box(), { type: "moved", url: "https://example.com/other" })).toBeNull();
    expect(pageRegionStep(comment(), { type: "moved", url: "https://example.com/other" })).toBeNull();
    expect(pageRegionStep(started(), { type: "moved", url: "https://example.com/other" })).toBeNull();
  });
  it("the page reporting the same address does not end it", () => {
    const s = box();
    expect(pageRegionStep(s, { type: "moved", url: URL })).toBe(s);
  });
  it("pageRegion_ends_when_the_tab_is_hidden", () => {
    for (const s of [started(), box(), comment()]) expect(pageRegionStep(s, { type: "hidden" })).toBeNull();
  });
  it("cancel and a delivered crop end it", () => {
    expect(pageRegionStep(box(), { type: "cancel" })).toBeNull();
    expect(pageRegionStep(comment(), { type: "added" })).toBeNull();
  });
  it("pageRegion_esc_in_the_comment_goes_back_to_the_box", () => {
    const failed = pageRegionStep(comment(), { type: "failed", error: "No." });
    expect(pageRegionStep(failed, { type: "back" })).toMatchObject({ step: "box", error: null });
    expect(pageRegionStep(box(), { type: "confirm" })).toMatchObject({ step: "comment" });
  });
  it("pageRegion_keeps_the_box_after_a_failed_add", () => {
    const before = comment();
    const busy = pageRegionStep(before, { type: "busy" });
    expect(busy).toMatchObject({ busy: true, error: null });
    const after = pageRegionStep(busy, { type: "failed", error: "Couldn't add the region." });
    expect(after).toMatchObject({ busy: false, error: "Couldn't add the region.", step: "comment", url: URL });
    expect(after?.region).toEqual(before?.region);
  });
  it("an event with no selection leaves none", () => {
    for (const e of [{ type: "picture", ok: true }, { type: "confirm" }, { type: "back" }, { type: "busy" }, { type: "failed", error: "x" }, { type: "moved", url: "x" }] as const) {
      expect(pageRegionStep(null, e)).toBeNull();
    }
  });
  it("starting again begins afresh", () => {
    expect(pageRegionStep(comment(), { type: "start", url: "https://b.test/" })).toMatchObject({ step: "wait", url: "https://b.test/", busy: false, error: null });
  });
});
