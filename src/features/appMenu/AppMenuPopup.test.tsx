import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { AppMenu } from "./AppMenuPopup";

const render = (doctorIssues?: number) =>
  renderToStaticMarkup(<AppMenu os="windows" anchor={null} doctorIssues={doctorIssues} onRun={() => {}} onClose={() => {}} />);

describe("AppMenu", () => {
  it("is a menu named Menu", () => {
    expect(render()).toContain('role="menu" aria-label="Menu"');
  });
  it("marks Settings as a closed submenu", () => {
    const h = render();
    expect(h).toContain('aria-haspopup="menu"');
    expect(h).toContain('aria-expanded="false"');
  });
  it("gives History its shortcut", () => {
    expect(render()).toContain('aria-keyshortcuts="Ctrl+Shift+Y"');
  });
  it("names Doctor with its count and hides the number", () => {
    const h = render(3);
    expect(h).toContain('aria-label="Doctor, 3 issues"');
    expect(h).toContain('<span class="app-menu__count" aria-hidden="true">');
  });
  it("shows no count before a check", () => {
    const h = render(undefined);
    expect(h).not.toContain("app-menu__count");
    expect(h).not.toContain("aria-label=\"Doctor");
  });
});
