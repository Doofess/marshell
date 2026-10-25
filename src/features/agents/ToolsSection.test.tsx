import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import type { ToolCatalog } from "../../generated/ToolCatalog";
import { ToolsView } from "./ToolsSection";

const catalog: ToolCatalog = {
  tools: [
    { id: "ssh", name: "SSH", own: false, presets: [{ id: "host", name: "Connect to a host", fields: [], found: true }] },
    { id: "mine", name: "Mine", own: true, presets: [{ id: "go", name: "Go now", fields: [], found: false }] },
  ],
  problems: [],
};

describe("ToolsView", () => {
  it("lists each tool with Shipped or Own and its presets, and the tools.json line", () => {
    const h = renderToStaticMarkup(<ToolsView catalog={catalog} />);
    expect(h).toContain("SSH");
    expect(h).toContain("Shipped");
    expect(h).toContain("Own");
    expect(h).toContain("Connect to a host");
    expect(h).toContain("Go now");
    expect(h).toContain("Add your own in tools.json in the Marshell folder. Settings gets an editor later.");
  });
  it("puts the file problem first and an entry's problem under that entry", () => {
    const h = renderToStaticMarkup(
      <ToolsView
        catalog={{
          ...catalog,
          problems: [
            { index: 0, id: null, reason: "tools.json can't be read: it isn't valid JSON (line 3)." },
            { index: 2, id: "mine", reason: "Its id is used twice." },
          ],
        }}
      />,
    );
    expect(h.indexOf("isn&#x27;t valid JSON")).toBeGreaterThan(-1);
    expect(h.indexOf("isn&#x27;t valid JSON")).toBeLessThan(h.indexOf("SSH"));
    expect(h.indexOf("Its id is used twice.")).toBeGreaterThan(h.indexOf("Mine"));
  });
  it("says a refused entry that is not listed, by its place", () => {
    const h = renderToStaticMarkup(<ToolsView catalog={{ ...catalog, problems: [{ index: 3, id: "gone", reason: "It has no presets." }] }} />);
    expect(h).toContain("It has no presets.");
    expect(h).toContain("gone");
  });
});
