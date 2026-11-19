import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { NotesBars, type NotesBarsProps } from "./NotesBars";

const none = () => {};
const base: NotesBarsProps = { name: "report.pdf", conflict: null, line: null, undo: false, copy: false, onUndo: none, onCopy: none, onReload: none, onOverwrite: none, onSave: none };
const html = (p: Partial<NotesBarsProps>) => renderToStaticMarkup(<NotesBars {...base} {...p} />);
/** The buttons' names, in order. */
const buttons = (markup: string) => [...markup.matchAll(/<button[^>]*>([^<]*)<\/button>/g)].map((m) => m[1]);

describe("NotesBars", () => {
  it("nothing to say draws nothing", () => {
    expect(html({})).toBe("");
  });

  it("the save line says the file was saved and offers Undo", () => {
    const m = html({ line: "Saved report.pdf.", undo: true });
    expect(m).toContain(">Saved report.pdf.<");
    expect(buttons(m)).toEqual(["Undo"]);
  });

  it("after Undo, or a save that can't be undone, the line has no button", () => {
    expect(buttons(html({ line: "report.pdf is back as it was before the save." }))).toEqual([]);
    expect(html({ line: "report.pdf changed after the save, so the save can't be undone." })).toContain("so the save can&#x27;t be undone.");
  });

  it("a refused save says the core's line and offers a copy", () => {
    const m = html({ line: "The file is read-only or in use by another program.", copy: true });
    expect(m).toContain(">The file is read-only or in use by another program.<");
    expect(buttons(m)).toEqual(["Save as a copy"]);
  });

  it("the changed bar: the line, then Save as a copy, Reload and Overwrite", () => {
    const m = html({ conflict: "changed", line: "Saved report.pdf.", undo: true });
    expect(m).toContain("report.pdf changed on disk. Your notes aren&#x27;t saved.");
    expect(buttons(m)).toEqual(["Save as a copy", "Reload", "Overwrite"]);
    // The bar stands in for the save line.
    expect(m).not.toContain("Saved report.pdf.");
  });

  it("the deleted bar: the line, then Save as a copy and Save", () => {
    const m = html({ conflict: "deleted" });
    expect(m).toContain("report.pdf was deleted. Your notes aren&#x27;t saved.");
    expect(buttons(m)).toEqual(["Save as a copy", "Save"]);
  });

  it("the bars take no focus: the person is reading or annotating", () => {
    for (const m of [html({ conflict: "changed" }), html({ conflict: "deleted" }), html({ line: "Saved report.pdf.", undo: true })]) {
      expect(m).not.toMatch(/autofocus|tabindex="-1"|aria-live|role="alert"|role="status"/i);
    }
  });

  it("the changed bar is a group named by its line", () => {
    const m = html({ conflict: "changed" });
    const id = /<p id="([^"]+)"/.exec(m)?.[1];
    expect(id).toBeTruthy();
    expect(m).toContain(`role="group" aria-labelledby="${id}"`);
  });
});
