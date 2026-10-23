import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { Detected } from "./Detected";
import type { Detection } from "./types";
import { DETECTIONS } from "./fixtures";

describe("Detected", () => {
  it("lists each CLI with its version, or Not installed", () => {
    const h = renderToStaticMarkup(<Detected detections={DETECTIONS.onlyClaude} />);
    expect(h).toContain("Claude Code");
    expect(h).toContain("2.1.296");
    expect(h).toContain("Not installed");
  });
  it("shows the whole path of a config under a folder with a space", () => {
    expect(renderToStaticMarkup(<Detected detections={DETECTIONS.allThree} />)).toContain("C:\\Users\\Home Office\\.claude\\settings.json");
  });
  it("offers a way forward when nothing is installed", () => {
    const h = renderToStaticMarkup(<Detected detections={DETECTIONS.none} />);
    expect(h).toContain("No coding agents found");
    expect(h).toContain("Continue with shells and tools");
  });
  it("says a file that does not parse, is read-only or is managed, in words", () => {
    expect(renderToStaticMarkup(<Detected detections={DETECTIONS.unparsable} />)).toContain("does not parse");
    expect(renderToStaticMarkup(<Detected detections={DETECTIONS.readOnly} />)).toContain("read-only");
    expect(renderToStaticMarkup(<Detected detections={DETECTIONS.managedPresent} />)).toContain("managed by your organisation");
  });
  it("shows a CLI without an adapter as Found, with its program and a note, and no version", () => {
    const later: Detection = { cli: "codex", installed: true, exePath: "C:/bin/codex.cmd", configPath: "", configState: "ok", later: true };
    const h = renderToStaticMarkup(<Detected detections={[...DETECTIONS.onlyClaude.slice(0, 1), later]} />);
    const row = h.slice(h.indexOf('data-agent="codex"'));
    expect(row).toContain("Found");
    expect(row).toContain("C:/bin/codex.cmd");
    expect(row).toContain("Live status and setup come in a later version.");
    expect(row).not.toMatch(/version [–\d]/);
  });
  it("shows a missing CLI without an adapter as Not installed, with no note", () => {
    const later: Detection = { cli: "antigravity", installed: false, configPath: "", configState: "ok", later: true };
    const h = renderToStaticMarkup(<Detected detections={[...DETECTIONS.onlyClaude.slice(0, 1), later]} />);
    const row = h.slice(h.indexOf('data-agent="antigravity"'));
    expect(row).toContain("Not installed");
    expect(row).not.toContain("detected__note");
  });
});
