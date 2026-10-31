import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { FIXTURES } from "../sidebar/fixtures";
import { SessionHeader } from "../window/SessionHeader";
import { ReferencesChip } from "./ReferencesChip";

const NOTE = "Marshell added a note: api is C:/code/api.";

describe("ReferencesChip", () => {
  it("says 1 reference for one name", () => {
    const html = renderToStaticMarkup(<ReferencesChip references={{ names: ["api"], note: NOTE }} />);
    expect(html).toContain(">1 reference<");
  });
  it("counts the names", () => {
    const html = renderToStaticMarkup(<ReferencesChip references={{ names: ["a", "b", "c"], note: NOTE }} />);
    expect(html).toContain(">3 references<");
  });
  it("is focusable and described by the note", () => {
    const html = renderToStaticMarkup(<ReferencesChip references={{ names: ["api"], note: NOTE }} />);
    expect(html).toContain('tabindex="0"');
    expect(html).toContain(`title="${NOTE}"`);
    const id = /aria-describedby="([^"]+)"/.exec(html)![1];
    expect(html).toMatch(new RegExp(`id="${id}"[^>]*>${NOTE}<`));
    expect(html).not.toContain("aria-live");
  });
  it("renders nothing without references", () => {
    expect(renderToStaticMarkup(<ReferencesChip references={null} />)).toBe("");
  });
  it("shows in the session header only when the session has references", () => {
    const row = FIXTURES.find((f) => f.id === "working")!;
    expect(renderToStaticMarkup(<SessionHeader row={row} />)).not.toContain("reference");
    expect(renderToStaticMarkup(<SessionHeader row={{ ...row, references: { names: ["api"], note: NOTE } }} />)).toContain("1 reference");
  });
});
