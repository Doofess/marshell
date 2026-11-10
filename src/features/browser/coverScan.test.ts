import { readdirSync, readFileSync } from "node:fs";
import { join, relative } from "node:path";
import { describe, expect, it } from "vitest";

const SRC = new URL("../..", import.meta.url).pathname.replace(/^\/([A-Za-z]:)/, "$1");

const walk = (dir: string): string[] =>
  readdirSync(dir, { withFileTypes: true }).flatMap((e) => (e.isDirectory() ? walk(join(dir, e.name)) : e.name.endsWith(".tsx") ? [join(dir, e.name)] : []));

/** Files that draw a menu or dialog and don't cover the page, each with the reason. */
const NOT_COVERING: Record<string, string> = {
  "features/peek/Peek.tsx": "a non-modal popover beside a sidebar row, never over the pane; a hover must not freeze the page",
  "features/window/AllSessionsMenu.tsx": "drawn only in a story so far; no app screen mounts it",
};

// A modal, a menu, a dialog, and the request-details drawer, which lies over the pane in a narrow window.
const DRAWS_OVER = /aria-modal="true"|role="menu"|role="dialog"|className="drawer"/;

describe("every overlay covers the page", () => {
  it("each .tsx that draws a modal, a menu, a dialog or the drawer calls useCoverPage(), or is listed with its reason", () => {
    const missing = walk(SRC)
      .filter((f) => !/\.(test|stories)\.tsx$/.test(f))
      .filter((f) => {
        const text = readFileSync(f, "utf8");
        return DRAWS_OVER.test(text) && !text.includes("useCoverPage(");
      })
      .map((f) => relative(SRC, f).replaceAll("\\", "/"))
      .filter((f) => !(f in NOT_COVERING))
      .sort();
    expect(missing).toEqual([]);
  });
});
