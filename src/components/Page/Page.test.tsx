import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { Page } from "./Page";

describe("Page", () => {
  it("is a labelled section with a heading, actions and a body", () => {
    const h = renderToStaticMarkup(<Page title="Doctor" subtitle="Checked 12 s ago" actions={<button>Run again</button>}>body</Page>);
    expect(h).toContain("<h2");
    expect(h).toContain("Doctor");
    expect(h).toContain("Run again");
    expect(h).toContain('class="page__body"');
    expect(h).toMatch(/<section[^>]*aria-labelledby="([^"]+)"/);
  });
  it("can be the page's h1", () => {
    expect(renderToStaticMarkup(<Page title="Settings" level={1}>x</Page>)).toContain("<h1");
  });
});
