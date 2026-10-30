import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { CommandList } from "./CommandList";

const count = (s: string, re: RegExp) => (s.match(re) ?? []).length;

describe("CommandList", () => {
  const cmds = ['git -C "C:/dev/my-app" worktree add -b marshell/x "C:/w/x" main', "git -C C:/w/x status"];
  const h = renderToStaticMarkup(<CommandList commands={cmds} />);
  it("is an ordered list labelled for a screen reader", () => {
    expect(h).toMatch(/^<ol[^>]*aria-label="Commands Marshell will run"/);
  });
  it("shows each command as its own code item", () => {
    expect(count(h, /<li[ >]/g)).toBe(2);
    expect(count(h, /<code[ >]/g)).toBe(2);
    expect(h).toContain("&quot;C:/dev/my-app&quot;");
  });
  it("renders hostile text as text", () => {
    expect(renderToStaticMarkup(<CommandList commands={["<img src=x onerror=alert(1)>"]} />)).not.toContain("<img");
  });
});

describe("CommandList with a teardown", () => {
  const h = renderToStaticMarkup(<CommandList commands={["git worktree remove w"]} teardown="pnpm run stop" />);
  it("shows the teardown as the first item, before the git commands", () => {
    expect(count(h, /<li[ >]/g)).toBe(2);
    expect(h.indexOf("teardown: pnpm run stop")).toBeGreaterThan(-1);
    expect(h.indexOf("teardown: pnpm run stop")).toBeLessThan(h.indexOf("git worktree remove w"));
  });
  it("renders a hostile script as text", () => {
    expect(renderToStaticMarkup(<CommandList commands={[]} teardown="<img src=x onerror=alert(1)>" />)).not.toContain("<img");
  });
});
