import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { CloseDialog } from "./CloseDialog";

const none = () => {};
const view = (props: Partial<Parameters<typeof CloseDialog>[0]> = {}) => renderToStaticMarkup(<CloseDialog busy={false} error={null} onChoose={none} onCancel={none} {...props} />);
const buttons = (h: string) => [...h.matchAll(/<button[^>]*>([^<]*)<\/button>/g)].map((m) => m[1]);

describe("CloseDialog", () => {
  it("is a modal dialog with an accessible name and the exact copy", () => {
    const h = view();
    expect(h).toMatch(/role="dialog"[^>]*aria-modal="true"/);
    expect(h).toMatch(/aria-labelledby="([^"]+)"/);
    const id = h.match(/aria-labelledby="([^"]+)"/)![1];
    expect(h).toContain(`id="${id}"`);
    expect(h).toContain("Marshell keeps running in the tray, so your sessions keep going.");
  });
  it("offers keeping it in the tray first, as the default, and then quitting", () => {
    const h = view();
    expect(buttons(h)).toEqual(["Keep running in the tray", "Quit when I close"]);
    expect(h.match(/<button[^>]*data-default[^>]*>Keep running in the tray</)).not.toBeNull();
    expect(h.match(/<button[^>]*data-default[^>]*>Quit when I close</)).toBeNull();
  });
  it("disables both buttons while the choice is being saved", () => {
    expect(view({ busy: true }).match(/<button[^>]*disabled=""/g)).toHaveLength(2);
    expect(view().match(/disabled=""/g)).toBeNull();
  });
  it("shows a failed save as an alert", () => {
    expect(view({ error: "Disk full." })).toMatch(/role="alert"[^>]*>Disk full\./);
  });
});
