import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import type { Encoding, Eol } from "./textFile";
import { Editor } from "./Editor";
import { EditorFooter } from "./EditorFooter";

const props = { name: "a.ts", path: "C:/p/a.ts", initial: "let a = 1;\n", readOnly: false, wrap: false, onEdit: () => {}, onCursor: () => {} };

describe("Editor", () => {
  it("renders_a_host_with_the_footers_hint_as_its_description", () => {
    const html = renderToStaticMarkup(
      <>
        <Editor {...props} hintId="hint-1" />
        <EditorFooter line={1} column={1} encoding="utf-8" eol="lf" hintId="hint-1" />
      </>,
    );
    expect(html).toContain('class="editor__host"');
    expect(html).toContain('data-hint="hint-1"');
    expect(html).toContain('id="hint-1"');
    expect(html).toContain("Esc, then Tab, leaves the editor");
  });

  it("the_footer_shows_the_position_the_encoding_and_the_ending", () => {
    const footer = (eol: Eol, encoding: Encoding) => renderToStaticMarkup(<EditorFooter line={12} column={5} encoding={encoding} eol={eol} />);
    const html = footer("crlf", "utf-8-bom");
    expect(html).toContain("Ln 12, Col 5");
    expect(html).toContain("UTF-8 with BOM");
    expect(html).toContain("CRLF");
    expect(footer("lf", "utf-16le")).toContain("UTF-16 LE");
    expect(footer("lf", "utf-16be")).toContain("UTF-16 BE");
    expect(footer("lf", "utf-8")).toContain(">LF<");
  });
});
