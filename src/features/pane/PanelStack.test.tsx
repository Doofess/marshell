import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import type { Layout } from "../window/paneColumn";
import { PanelStack, dividerKey, dividerProps, rowLayout } from "./PanelStack";
import type { PanelId } from "./panels";

const noop = () => {};
const render = (panel: PanelId) => <p data-frame={panel}>{panel} frame</p>;
const html = (panels: PanelId[], weights = {}) => renderToStaticMarkup(<PanelStack panels={panels} weights={weights} render={render} onWeights={noop} />);

describe("PanelStack", () => {
  it("draws a row per panel, in order, each holding its frame", () => {
    const out = html(["browser", "files"]);
    expect(out.match(/class="panel-stack__row"/g)).toHaveLength(2);
    expect(out.indexOf('data-panel="browser"')).toBeLessThan(out.indexOf('data-panel="files"'));
    expect(out).toMatch(/data-panel="browser"[^>]*><p data-frame="browser">browser frame<\/p>/);
    expect(out).toMatch(/data-panel="files"[^>]*><p data-frame="files">files frame<\/p>/);
  });
  it("is the pane's drop target", () => {
    expect(html(["files"])).toMatch(/^<div class="panel-stack" data-drop-pane=""/);
  });
  it("before it is measured the rows share the height by weight and the divider is inert", () => {
    const out = html(["browser", "files"], { browser: 30000, files: 10000 });
    expect(out).toContain("grid-template-rows:minmax(0, 30000fr) 1px minmax(0, 10000fr)");
    const divider = out.match(/<div[^>]*class="panel-stack__divider"[^>]*>/)![0];
    expect(divider).toContain('aria-hidden="true"');
    expect(divider).not.toContain("tabindex");
    expect(divider).not.toContain("role=");
  });
  it("a weight that is absent counts as the default", () => {
    expect(html(["browser", "files"])).toContain("grid-template-rows:minmax(0, 10000fr) 1px minmax(0, 10000fr)");
  });
  it("one panel draws no divider track", () => {
    const out = html(["files"]);
    expect(out).not.toContain("panel-stack__divider");
    expect(out).toContain("grid-template-rows:minmax(0, 10000fr)");
    expect(out).not.toContain("1px");
  });
  it("the divider names both panels and reports the upper one's height in pixels", () => {
    expect(dividerProps("browser", "files", 440, 160)).toEqual({ "aria-label": "Resize Browser and Files", "aria-valuemin": 160, "aria-valuemax": 440, "aria-valuenow": 440 });
  });
  it("Up and Down move 16 px, Home and End go to the minimum of each side, any other key is null", () => {
    expect(dividerKey("ArrowUp", 300, 300)).toEqual({ upper: 284, lower: 316 });
    expect(dividerKey("ArrowDown", 300, 300)).toEqual({ upper: 316, lower: 284 });
    expect(dividerKey("Home", 300, 300)).toEqual({ upper: 160, lower: 440 });
    expect(dividerKey("End", 300, 300)).toEqual({ upper: 440, lower: 160 });
    expect(dividerKey("ArrowUp", 160, 440)).toEqual({ upper: 160, lower: 440 });
    expect(dividerKey("ArrowLeft", 300, 300)).toBeNull();
    expect(dividerKey("a", 300, 300)).toBeNull();
  });
  it("gives the page's slot one layout object per state", () => {
    const outer = { inputs: { pref: "expanded", drawer: false, shown: true, saved: null, terminalMin: 400, rtl: false }, moving: false } as Layout;
    expect(rowLayout(null, true, 0.5)).toBeNull();
    expect(rowLayout(outer, false, 0)).toBe(outer);
    expect(rowLayout(outer, true, 0)).toEqual({ ...outer, moving: true, bottomPerPx: 0 });
    expect(rowLayout(outer, false, 0.5)).toEqual({ ...outer, moving: false, bottomPerPx: 0.5 });
    expect(rowLayout({ ...outer, moving: true }, false, 0.25)).toEqual({ ...outer, moving: true, bottomPerPx: 0.25 });
  });
});
