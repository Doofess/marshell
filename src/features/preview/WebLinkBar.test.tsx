import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { WebLinkBar } from "./WebLinkBar";

const noop = () => {};
const text = (markup: string) => markup.replace(/<[^>]*>/g, "");

describe("WebLinkBar", () => {
  it("asks the question with the whole address in it", () => {
    const url = "https://example.com/" + "very-long-path/".repeat(20) + "?q=1&r=2#frag";
    const out = renderToStaticMarkup(<WebLinkBar url={url} onOpen={noop} onCancel={noop} />);
    expect(text(out)).toContain(`Open ${url.replace(/&/g, "&amp;")} in your browser?`);
  });
  it("has Open and Cancel buttons, and names the group by the question", () => {
    const out = renderToStaticMarkup(<WebLinkBar url="https://example.com/" onOpen={noop} onCancel={noop} />);
    expect(out).toMatch(/>Open<\/button>/);
    expect(out).toMatch(/>Cancel<\/button>/);
    const id = /aria-labelledby="([^"]+)"/.exec(out)?.[1];
    expect(id).toBeTruthy();
    expect(out).toContain(`id="${id}"`);
  });
});
