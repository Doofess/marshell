import { readFileSync } from "node:fs";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { QuitDialog, quitStopFailed } from "./QuitDialog";

const none = () => {};
const view = (props: Partial<Parameters<typeof QuitDialog>[0]> = {}) => renderToStaticMarkup(<QuitDialog count={1} busy={false} error={null} onQuit={none} onCancel={none} {...props} />);
const buttons = (h: string) => [...h.matchAll(/<button[^>]*>([^<]*)<\/button>/g)].map((m) => m[1]);

describe("QuitDialog", () => {
  it("asks about one push and about several", () => {
    expect(view({ count: 1 })).toContain("A push is still running. Quit and stop it?");
    expect(view({ count: 2 })).toContain("2 pushes are still running. Quit and stop them?");
    expect(view({ count: 12 })).toContain("12 pushes are still running. Quit and stop them?");
  });
  it("offers Cancel first", () => {
    const h = view();
    expect(buttons(h)).toEqual(["Cancel", "Quit"]);
    expect(h.match(/<button[^>]*data-default[^>]*>Cancel</)).not.toBeNull();
    expect(h.match(/<button[^>]*data-default[^>]*>Quit</)).toBeNull();
  });
  it("is a modal dialog named by its title and described by the sentence", () => {
    const h = view();
    expect(h).toMatch(/role="dialog"[^>]*aria-modal="true"/);
    expect(h).toContain("Quit Marshell?");
    const dialog = h.match(/<div[^>]*role="dialog"[^>]*>/)![0];
    const named = dialog.match(/aria-labelledby="([^"]+)"/)![1];
    const described = dialog.match(/aria-describedby="([^"]+)"/)![1];
    expect(h).toMatch(new RegExp(`<h2[^>]*id="${named}"[^>]*>Quit Marshell\\?<`));
    expect(h).toMatch(new RegExp(`<p[^>]*id="${described}"[^>]*>A push is still running`));
  });
  it("keeps both buttons in the tab order but off while the pushes are being stopped", () => {
    const busy = view({ busy: true });
    expect(busy.match(/<button[^>]*aria-disabled="true"/g)).toHaveLength(2);
    expect(busy.match(/<button[^>]*\sdisabled=""/g)).toBeNull();
    expect(view().match(/aria-disabled/g)).toBeNull();
  });
  it("shows a failed stop as a sentence that can take focus, not as a second live region", () => {
    const h = view({ error: quitStopFailed(1) });
    expect(h).toMatch(/<p[^>]*tabindex="-1"[^>]*data-focus=""[^>]*>Couldn&#x27;t stop the push\. Try again\.<\/p>/);
    expect(h).not.toMatch(/role="alert"|aria-live/);
    expect(view().match(/data-focus/g)).toBeNull();
  });
  it("words the failure in the number of pushes", () => {
    expect(quitStopFailed(1)).toBe("Couldn't stop the push. Try again.");
    expect(quitStopFailed(2)).toBe("Couldn't stop the pushes. Try again.");
  });
  it("styles its off buttons only inside its own sheet, not in every close dialog", () => {
    const css = readFileSync(new URL("./QuitDialog.css", import.meta.url), "utf8");
    expect(css.split(String.fromCharCode(10)).filter((line) => line.includes("[aria-disabled") && !line.trimStart().startsWith(".quit-dialog "))).toEqual([]);
    expect(renderToStaticMarkup(<QuitDialog count={1} busy error={null} onQuit={() => {}} onCancel={() => {}} />)).toContain('class="close-dialog__sheet quit-dialog"');
  });
});
