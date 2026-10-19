import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { nameContainsLabel } from "../../lib/labelInName";
import type { RowModel } from "../sidebar/types";
import { Peek } from "./Peek";

const row = { id: "t6", number: 6, name: "api-server" } as RowModel;
const html = (rows: string[] | null, failed = false) => renderToStaticMarkup(<Peek row={row} rows={rows} failed={failed} onClose={() => {}} />);

describe("Peek", () => {
  it("is a non-modal dialog named for the session, holding the rows joined by newlines in a focusable pre", () => {
    const h = html(["$ npm test", "ok"]);
    expect(h).toContain('role="dialog"');
    expect(h).toContain('aria-label="Peek: Session 6, api-server"');
    expect(h).not.toContain("aria-modal");
    expect(h).toMatch(/<pre[^>]*tabindex="0"[^>]*>\$ npm test\nok<\/pre>/);
  });
  it("shows nothing while the rows are loading", () => {
    expect(html(null)).toBe("");
  });
  it("says so for an empty screen and for a failed read", () => {
    expect(html([])).toContain("Nothing on screen yet");
    expect(html(["", "  "])).toContain("Nothing on screen yet");
    expect(html(null, true)).toContain("Can&#x27;t read this session&#x27;s screen");
  });
  it("keeps the dialog's name containing what its controls show (the pre has no label of its own)", () => {
    const pre = /<pre[\s\S]*<\/pre>/.exec(html(["x"]))![0];
    expect(nameContainsLabel(pre)).toBe(true);
  });
});
