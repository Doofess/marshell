import { readFileSync } from "node:fs";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { ClearDataDialog } from "./ClearDataDialog";

const none = () => {};
const view = (props: Partial<Parameters<typeof ClearDataDialog>[0]> = {}) => renderToStaticMarkup(<ClearDataDialog busy={false} onClear={none} onCancel={none} {...props} />);
const buttons = (h: string) => [...h.matchAll(/<button[^>]*>([^<]*)<\/button>/g)].map((m) => m[1]);

describe("ClearDataDialog", () => {
  it("is a modal dialog named by its heading, with the exact texts", () => {
    const h = view();
    expect(h).toMatch(/role="dialog"[^>]*aria-modal="true"/);
    const id = h.match(/aria-labelledby="([^"]+)"/)![1];
    expect(h).toMatch(new RegExp(`<h2[^>]*id="${id}"[^>]*>Clear the browser&#x27;s data\\?</h2>`));
    expect(h).toContain("This signs you out of every site in Marshell&#x27;s browser and clears its cookies, cache and site permissions. Your own browser isn&#x27;t touched.");
    expect(buttons(h)).toEqual(["Clear data", "Cancel"]);
  });
  it("describes the dialog with the body text", () => {
    const h = view();
    const id = h.match(/aria-describedby="([^"]+)"/)![1];
    expect(h).toMatch(new RegExp(`<p[^>]*id="${id}"[^>]*>This signs you out`));
  });
  it("disables both buttons while the data is being cleared", () => {
    expect(view({ busy: true }).match(/<button[^>]*disabled=""/g)).toHaveLength(2);
  });
  it("covers the page while it is drawn", () => {
    expect(readFileSync(new URL("./ClearDataDialog.tsx", import.meta.url), "utf8")).toContain("useCoverPage(");
  });
});
