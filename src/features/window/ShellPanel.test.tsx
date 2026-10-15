import { readFileSync } from "node:fs";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { ShellPanel, shellNameFor } from "./ShellPanel";

const html = (p: Partial<Parameters<typeof ShellPanel>[0]> = {}) => renderToStaticMarkup(<ShellPanel height={284} cwd="my-app" {...p} />);

describe("ShellPanel", () => {
  it("is a labelled section named Shell, with a real terminal inside", () => {
    const h = html();
    expect(h).toContain("<section");
    expect(h).toContain('aria-label="Shell"');
    expect(h).toContain('class="scripted-terminal"');
    expect(h).toContain('aria-label="PowerShell terminal"');
  });
  it("has a 32 px header with the shell's name and the working folder", () => {
    const h = html();
    expect(h).toContain("PowerShell");
    expect(h).toContain(">my-app<");
  });
  it("leaves the folder out when there is none", () => {
    expect(html({ cwd: undefined })).not.toContain("shell-panel__cwd");
  });
  it("names the shell for the operating system", () => {
    expect(shellNameFor("windows")).toBe("PowerShell");
    expect(shellNameFor("mac")).toBe("zsh");
    expect(shellNameFor("linux")).toBe("bash");
    expect(html({ shellName: "zsh" })).toContain("zsh");
    expect(html({ shellName: "zsh" })).toContain('aria-label="zsh terminal"');
  });
  it("is not an agent: no vendor, so the user's own accent applies, and nothing about it targets a session", () => {
    const h = html();
    expect(h).not.toContain("data-agent");
    expect(h).not.toContain("shell-panel__stripe");
    expect(h).not.toContain("composer");
  });
  it("opens in a pane with a button named for what it does", () => {
    expect(html()).toContain('aria-label="Open PowerShell in a pane"');
    expect(html({ shellName: "bash" })).toContain('aria-label="Open bash in a pane"');
  });
  it("hides and shows with one toggle that keeps one name; its state is aria-expanded, and its tooltip says what it will do", () => {
    const open = html();
    expect(open).toMatch(/aria-label="Shell panel"[^>]*aria-expanded="true"|aria-expanded="true"[^>]*aria-label="Shell panel"/);
    expect(open).toContain('title="Hide the shell');
    const shut = html({ collapsed: true, height: 32 });
    expect(shut).toMatch(/aria-label="Shell panel"[^>]*aria-expanded="false"|aria-expanded="false"[^>]*aria-label="Shell panel"/);
    expect(shut).toContain('title="Show the shell');
  });
  it("shows only the header when collapsed, and says when it collapsed itself", () => {
    const shut = html({ collapsed: true, height: 32 });
    expect(shut).not.toContain("scripted-terminal");
    expect(shut).toContain('data-collapsed="true"');
    expect(html({ collapsed: true, auto: true, height: 32 })).toContain('data-auto="true"');
    expect(html()).not.toContain('data-collapsed');
  });
  it("sets its own height, from the layout", () => {
    expect(html({ height: 300 })).toContain("block-size:300px");
  });
  it("puts the shortcut in the toggle's tooltip, per operating system", () => {
    expect(html()).toContain("Ctrl+Shift+S");
    expect(html({ os: "mac" })).toContain("⌘S");
  });
  it("carries no literal colour in its markup", () => {
    expect(html()).not.toMatch(/#[0-9a-f]{3,8}\b|rgb\(|oklch\(/i);
  });
});

describe("ShellPanel.css", () => {
  const css = readFileSync(new URL("./ShellPanel.css", import.meta.url), "utf8");
  const rule = (s: string) => {
    const i = css.indexOf(`${s} {`);
    return i < 0 ? "" : css.slice(i, css.indexOf("}", i));
  };
  it("is a 32 px header over the terminal", () => {
    expect(rule(".shell-panel")).toMatch(/grid-template-rows:\s*32px/);
  });
  it("makes the header buttons 24 px, the minimum target", () => {
    expect(rule(".shell-panel__button")).toContain("inline-size: 24px");
    expect(rule(".shell-panel__button")).toContain("block-size: 24px");
  });
  it("styles focus and press, and hover only for a fine pointer", () => {
    expect(css).toContain(".shell-panel__button:focus-visible");
    expect(css).toContain(".shell-panel__button:active");
    expect(css).toMatch(/@media \(hover: hover\) and \(pointer: fine\)/);
  });
  it("uses tokens and logical properties only, and no boxes", () => {
    expect(css).not.toMatch(/#[0-9a-f]{3,8}\b|rgb\(/i);
    expect(css).not.toMatch(/\b(margin|padding|border)-(left|right|top|bottom)\b/);
    expect(css).not.toMatch(/\b(left|right|top|bottom):/);
  });
  it("never scrolls itself: the terminal inside has its own scrollback", () => {
    expect(css).not.toMatch(/overflow(-[xy])?:\s*(auto|scroll)/);
  });
});
