import { isValidElement, type ReactElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it, vi } from "vitest";
import { FIXTURES } from "../sidebar/fixtures";
import { TerminalStack, nextVisited } from "./keptTerminals";

// There is no DOM in these tests, so "stays mounted" is checked the way React decides it: a view stays mounted
// while it keeps the same key at the same place, and unmounts when it is no longer rendered.
const ids = ["a", "b", "c"];
const rows = ids.map((id) => ({ ...FIXTURES[0]!, id }));

/** The wrappers a render produces: their keys, and which of them are hidden. */
function wrappers(visited: string[], selectedId: string, render = (row: { id: string }) => <i data-view={row.id} />) {
  const stack = TerminalStack({ rows, visited, selectedId, render });
  const list = stack.props.children as ReactElement<{ hidden: boolean }>[];
  return list.filter(isValidElement).map((el) => ({ key: el.key, hidden: el.props.hidden }));
}

describe("nextVisited", () => {
  it("starts with the selected session", () => {
    expect(nextVisited([], ids, "a")).toEqual(["a"]);
  });
  it("keeps a session it has seen when the selection moves away and back", () => {
    const afterB = nextVisited(nextVisited([], ids, "a"), ids, "b");
    expect(afterB).toEqual(["a", "b"]);
    expect(nextVisited(afterB, ids, "a")).toEqual(["a", "b"]);
  });
  it("drops a session that left the list", () => {
    expect(nextVisited(["a", "b"], ["a", "c"], "a")).toEqual(["a"]);
  });
  it("adds nothing for no selection or an unknown one", () => {
    expect(nextVisited(["a"], ids, null)).toEqual(["a"]);
    expect(nextVisited(["a"], ids, "zzz")).toEqual(["a"]);
  });
});

describe("TerminalStack", () => {
  it("shows the selected view and hides the others without unmounting them", () => {
    expect(wrappers(["a", "b"], "b")).toEqual([
      { key: "a", hidden: true },
      { key: "b", hidden: false },
    ]);
  });
  it("switching away and back keeps the same keys in the same places, so React keeps every view mounted", () => {
    const first = wrappers(["a"], "a");
    const onB = wrappers(["a", "b"], "b");
    const backOnA = wrappers(["a", "b"], "a");
    expect(first.map((w) => w.key)).toEqual(["a"]);
    expect(onB.map((w) => w.key)).toEqual(["a", "b"]);
    expect(backOnA.map((w) => w.key)).toEqual(onB.map((w) => w.key));
    expect(backOnA).toEqual([
      { key: "a", hidden: false },
      { key: "b", hidden: true },
    ]);
  });
  it("renders a view only for a visited session, and tells it whether it is on show", () => {
    const render = vi.fn((row: { id: string }, shown: boolean) => <i data-view={row.id} data-shown={shown} />);
    const html = renderToStaticMarkup(<TerminalStack rows={rows} visited={["a", "c"]} selectedId="c" render={render} />);
    expect(render.mock.calls.map(([row, shown]) => [row.id, shown])).toEqual([
      ["a", false],
      ["c", true],
    ]);
    expect(html).toContain('data-view="a"');
    expect(html).not.toContain('data-view="b"');
    // The hidden one is still in the page, with the hidden attribute.
    expect(html).toMatch(/<div[^>]*\bhidden\b[^>]*><i data-view="a"/);
  });
  it("keeps every terminal mounted but shows none while suspended, as when the preview fills the window", () => {
    const render = vi.fn((row: { id: string }, shown: boolean) => <i data-view={row.id} data-shown={shown} />);
    const html = renderToStaticMarkup(<TerminalStack rows={rows} visited={["a", "c"]} selectedId="c" render={render} suspended />);
    expect(render.mock.calls.map(([row, shown]) => [row.id, shown])).toEqual([
      ["a", false],
      ["c", false],
    ]);
    expect(html.match(/ hidden=/g)).toHaveLength(2);
  });
  it("renders nothing for a visited session that is no longer in the list", () => {
    expect(wrappers(["a", "gone"], "a")).toEqual([{ key: "a", hidden: false }]);
  });
});
