import { readFileSync } from "node:fs";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { SaveDialog } from "./SaveDialog";

const view = (name = "a.txt") => renderToStaticMarkup(<SaveDialog name={name} onChoose={() => {}} />);
const buttons = (h: string) => [...h.matchAll(/<button[^>]*>([^<]*)<\/button>/g)].map((m) => m[1]);

describe("SaveDialog", () => {
  it("it_has_the_plans_copy_and_three_buttons", () => {
    const h = view("notes.md");
    expect(h).toMatch(/<h2[^>]*>Save the changes to notes\.md\?<\/h2>/);
    expect(h).toContain("Its unsaved edits are lost if you don&#x27;t save them.");
    expect(buttons(h)).toEqual(["Save", "Don&#x27;t save", "Cancel"]);
    // Save is the default choice.
    expect(h).toMatch(/<button[^>]*data-default[^>]*>Save<\/button>/);
  });
  it("it_is_a_modal_dialog_named_by_its_title", () => {
    const h = view();
    expect(h).toMatch(/role="dialog"[^>]*aria-modal="true"/);
    const id = h.match(/aria-labelledby="([^"]+)"/)![1];
    expect(h).toMatch(new RegExp(`<h2[^>]*id="${id}"`));
    const described = h.match(/aria-describedby="([^"]+)"/)![1];
    expect(h).toMatch(new RegExp(`<p[^>]*id="${described}"[^>]*>Its unsaved edits`));
  });
  it("covers the page while it is drawn", () => {
    expect(readFileSync(new URL("./SaveDialog.tsx", import.meta.url), "utf8")).toContain("useCoverPage(");
  });
});
