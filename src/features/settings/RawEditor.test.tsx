import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { RawEditor } from "./RawEditor";
import { SETTINGS_TEXT, SETTINGS_TEXT_BROKEN } from "./fixtures";

describe("RawEditor", () => {
  it("is a labelled monospace text area holding the file text", () => {
    const h = renderToStaticMarkup(<RawEditor path={"C:\\Users\\Home Office\\.claude\\settings.json"} text={SETTINGS_TEXT} />);
    expect(h).toContain("<textarea");
    expect(h).toContain('aria-label="C:\\Users\\Home Office\\.claude\\settings.json"');
    expect(h).toMatch(/spellcheck="false"/i); // React writes spellCheck; HTML attribute names are case-insensitive
  });
  it("reports a mistake with its line, linked to the text area by aria-describedby", () => {
    const h = renderToStaticMarkup(<RawEditor path="settings.json" text={SETTINGS_TEXT_BROKEN} />);
    expect(h).toMatch(/Line 3, column \d+:/);
    expect(h).toMatch(/aria-describedby="([^"]+)"/);
    expect(h).toContain('aria-invalid="true"');
  });
  it("has no description and is not invalid when the text parses", () => {
    const h = renderToStaticMarkup(<RawEditor path="settings.json" text={SETTINGS_TEXT} />);
    expect(h).not.toContain("aria-describedby");
    expect(h).not.toContain("aria-invalid");
  });
  it("is read-only when the file must not be edited", () => {
    const h = renderToStaticMarkup(<RawEditor path="settings.json" text={SETTINGS_TEXT_BROKEN} readOnly />);
    expect(h).toMatch(/<textarea[^>]*readOnly=""|<textarea[^>]*readonly=""/i);
  });
});
