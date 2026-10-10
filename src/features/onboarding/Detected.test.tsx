import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { Detected } from "./Detected";
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
});
