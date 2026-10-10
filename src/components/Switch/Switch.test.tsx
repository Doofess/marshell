import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { Switch } from "./Switch";

describe("Switch", () => {
  it("is a switch whose state is a word, so it never depends on colour", () => {
    const on = renderToStaticMarkup(<Switch label="Approve from anywhere" checked />);
    expect(on).toContain('role="switch"');
    expect(on).toContain("checked");
    expect(on).toContain(">On<");
    expect(renderToStaticMarkup(<Switch label="Approve from anywhere" checked={false} />)).toContain(">Off<");
  });
  it("explains why it is unavailable and names the reason in the control's description", () => {
    const h = renderToStaticMarkup(<Switch label="Approve from anywhere" checked={false} disabled reason="Gemini has no approval hook, so Marshell focuses the tab instead." />);
    expect(h).toContain("disabled");
    expect(h).toContain("Gemini has no approval hook");
    expect(h).toMatch(/aria-describedby="([^"]+)"/);
  });
});

describe("Switch: state words", () => {
  it("keeps the state words out of the accessible name, and shows the one that matches the control's own state", () => {
    const h = renderToStaticMarkup(<Switch label="Play sounds" checked />);
    expect(h).toMatch(/class="switch__state" aria-hidden="true"/);
    expect(h).toContain(">On<");
    expect(h).toContain(">Off<");
  });
  it("says the state is unknown with an en dash instead of guessing", () => {
    const h = renderToStaticMarkup(<Switch label="Play sounds" checked={false} unknown disabled />);
    expect(h).toContain("data-unknown");
    expect(h).toContain(">–<");
  });
});
