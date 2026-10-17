import { isValidElement, type ReactElement, type ReactNode } from "react";
import { describe, expect, it, vi } from "vitest";

// These tests need no DOM: the components are called as plain functions and the element tree they return is searched.
// Their hooks do nothing here, which is enough because nothing below the top call is rendered.
vi.mock("react", async (importOriginal) => {
  const actual = await importOriginal<typeof import("react")>();
  return { ...actual, useEffect: () => {}, useRef: (init: unknown) => ({ current: init }), useState: (init: unknown) => [typeof init === "function" ? init() : init, () => {}] };
});

const { FIXTURES } = await import("../sidebar/fixtures");
const { SessionRow } = await import("../sidebar/SessionRow");
const { MainWindow } = await import("./MainWindow");
const { Rail } = await import("./Rail");
const { Sidebar } = await import("./Sidebar");

const rows = ["needs-permission", "working", "done-unseen"].map((id) => FIXTURES.find((f) => f.id === id)!);

function findAll(node: ReactNode, match: (el: ReactElement<Record<string, unknown>>) => boolean, found: ReactElement<Record<string, unknown>>[] = []) {
  if (Array.isArray(node)) node.forEach((n) => findAll(n, match, found));
  else if (isValidElement<Record<string, unknown>>(node)) {
    if (match(node)) found.push(node);
    findAll(node.props.children as ReactNode, match, found);
  }
  return found;
}
const nth = <T,>(list: T[], i: number): T => {
  const item = list[i];
  if (item === undefined) throw new Error(`nothing at ${i}`);
  return item;
};
const button = (tree: ReactNode, label: string) => nth(findAll(tree, (el) => el.type === "button" && el.props["aria-label"] === label), 0);
const row = (i: number) => nth(rows, i);

describe("pointer: switching sessions", () => {
  it("a click on a sidebar row selects that session", () => {
    const onSelect = vi.fn();
    const tree = Sidebar({ rows, requests: [], height: 700, width: 288, onSelect });
    const sessionRows = findAll(tree, (el) => el.type === SessionRow);
    expect(sessionRows).toHaveLength(3);
    (nth(sessionRows, 1).props.onSelect as () => void)();
    expect(onSelect).toHaveBeenCalledExactlyOnceWith(row(1).id);
  });

  it("a row calls onSelect when it is clicked, and is inert without one", () => {
    const onSelect = vi.fn();
    const root = SessionRow({ row: row(0), density: "comfortable", onSelect });
    (root.props.onClick as () => void)();
    expect(onSelect).toHaveBeenCalledTimes(1);
    expect(SessionRow({ row: row(0), density: "comfortable" }).props.onClick).toBeUndefined();
  });

  it("a click on a rail button selects that session", () => {
    const onSelect = vi.fn();
    const tree = Rail({ rows, needsYou: 0, onSelect });
    const buttons = findAll(tree, (el) => el.type === "button" && String(el.props.className).startsWith("rail__item"));
    expect(buttons).toHaveLength(3);
    (nth(buttons, 2).props.onClick as () => void)();
    expect(onSelect).toHaveBeenCalledExactlyOnceWith(row(2).id);
  });

  it("MainWindow hands its onSelect to the sidebar and to the rail", () => {
    const onSelect = vi.fn();
    const wide = MainWindow({ width: 1180, height: 800, rows, selectedId: row(0).id, onSelect });
    expect(nth(findAll(wide, (el) => el.type === Sidebar), 0).props.onSelect).toBe(onSelect);
    const narrow = MainWindow({ width: 1180, height: 800, layout: "rail", rows, selectedId: row(0).id, onSelect });
    expect(nth(findAll(narrow, (el) => el.type === Rail), 0).props.onSelect).toBe(onSelect);
  });
});

describe("pointer: the command palette button", () => {
  it("the header button calls onPalette", () => {
    const onPalette = vi.fn();
    const tree = MainWindow({ width: 1180, height: 800, rows, selectedId: row(0).id, onPalette });
    (button(tree, "Command palette").props.onClick as () => void)();
    expect(onPalette).toHaveBeenCalledTimes(1);
  });

  it("the palette button stays in the bar in the rail, and calls onPalette", () => {
    const onPalette = vi.fn();
    (button(MainWindow({ width: 1180, height: 800, rows, selectedId: row(0).id, sidebarPref: "rail", onPalette }), "Command palette").props.onClick as () => void)();
    expect(onPalette).toHaveBeenCalledTimes(1);
  });

  it("the sidebar button calls onToggleSidebar, unless a narrow window blocks it", () => {
    const onToggleSidebar = vi.fn();
    (button(MainWindow({ width: 1180, height: 800, rows, selectedId: row(0).id, onToggleSidebar }), "Sidebar").props.onClick as () => void)();
    expect(onToggleSidebar).toHaveBeenCalledTimes(1);
    expect(button(MainWindow({ width: 900, height: 800, rows, selectedId: row(0).id, onToggleSidebar }), "Sidebar").props.onClick).toBeUndefined();
  });

  it("without a callback the buttons stay as they were (stories)", () => {
    const tree = MainWindow({ width: 1180, height: 800, rows, selectedId: row(0).id });
    expect(button(tree, "Command palette").props.onClick).toBeUndefined();
  });
});
