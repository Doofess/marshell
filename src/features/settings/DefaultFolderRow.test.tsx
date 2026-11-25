import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import type { DefaultFolder } from "../../generated/DefaultFolder";
import { DefaultFolderRowView } from "./DefaultFolderRow";

const own: DefaultFolder = { path: "C:\\Users\\u\\Marshell", exists: true, chosen: false };
const mine: DefaultFolder = { path: "D:\\Work\\Start", exists: true, chosen: true };
const noop = () => {};
const html = (p: Partial<Parameters<typeof DefaultFolderRowView>[0]> = {}) => renderToStaticMarkup(<DefaultFolderRowView folder={own} error={null} busy={false} onChoose={noop} onReset={noop} {...p} />);

describe("DefaultFolderRowView", () => {
  it("shows the full path, the label and the help line", () => {
    const h = html();
    expect(h).toContain("C:\\Users\\u\\Marshell");
    expect(h).toContain("Default folder");
    expect(h).toContain("Where the shell panel and new sessions start when no folder is chosen and the shell profile sets none. Marshell makes it when it&#x27;s missing.");
    expect(h).toContain("Choose folder…");
  });
  it("says Marshell makes the folder only for its own", () => {
    const makes = "Marshell makes it when it&#x27;s missing.";
    const where = "Where the shell panel and new sessions start when no folder is chosen and the shell profile sets none.";
    for (const folder of [mine, { ...mine, exists: false }, { path: null, exists: false, chosen: false }, null]) {
      const h = html({ folder });
      expect(h).toContain(where);
      expect(h).not.toContain(makes);
    }
    expect(html({ folder: { ...own, exists: false } })).toContain(makes);
  });
  it("shows the reset button only for a chosen folder", () => {
    expect(html()).not.toContain("Use Marshell&#x27;s folder");
    expect(html({ folder: mine })).toContain("Use Marshell&#x27;s folder");
  });
  it("says the folder is not there", () => {
    const h = html({ folder: { ...mine, exists: false } });
    expect(h).toContain("This folder isn&#x27;t there now, so shells and sessions start in your home folder.");
    expect(html()).not.toContain("isn&#x27;t there now");
  });
  it("shows the core's refusal", () => {
    expect(html({ error: "That folder isn't there." })).toContain("That folder isn&#x27;t there.");
  });
  it("says when the folder could not be read", () => {
    const h = html({ folder: null, error: "Couldn't read the default folder. Try again in a moment." });
    expect(h).toContain("–");
    expect(h).toContain("Couldn&#x27;t read the default folder. Try again in a moment.");
  });
  it("reads Your home folder when the core has none", () => {
    expect(html({ folder: { path: null, exists: false, chosen: false } })).toContain("Your home folder");
  });
  it("disables both buttons while busy", () => {
    const h = html({ folder: mine, busy: true });
    expect(h.match(/<button[^>]*disabled/g)).toHaveLength(2);
    expect(html({ folder: mine }).match(/<button[^>]*disabled/g)).toBeNull();
  });
  it("sits in its own block, so it is set apart from the profiles below", () => {
    expect(html()).toMatch(/^<div class="default-folder">.*<\/div>$/s);
  });
  it("the group is labelled by Default folder", () => {
    const h = html();
    const id = /role="group" aria-labelledby="([^"]+)"/.exec(h)?.[1];
    expect(id).toBeTruthy();
    expect(h).toMatch(new RegExp(`id="${id}"[^>]*>Default folder<`));
  });
});
