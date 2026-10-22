import { afterEach, describe, expect, it, vi } from "vitest";
import { elements, mountHook } from "../../test/hooks";
import { APPROVALS } from "../approval/fixtures";
import type { ApprovalRequest } from "../approval/types";
import type { RowModel } from "../sidebar/types";
import { FIXTURES } from "../sidebar/fixtures";
import { boardRows, splitGroups } from "./board";
import { activeIndex, cardFor, fitLiveBoard, focusAfter, gateRows, heldRows, openerUsable, refocusNeeded } from "./liveBoard";
import { LiveDepartures } from "./LiveDepartures";
import { DeparturesBoard } from "./DeparturesBoard";

vi.mock("react", async () => {
  const { hooks } = await import("../../test/hooks");
  return { ...await vi.importActual("react"), ...hooks, useLayoutEffect: hooks.useEffect };
});
afterEach(() => vi.unstubAllGlobals());

const row = (id: string, number: number): RowModel => ({ ...FIXTURES[2]!, id, number, status: "idle" });
const request = (tab: string, waitingMs = 60_000): ApprovalRequest => ({ ...APPROVALS.safeBash, id: `${tab}-${waitingMs}`, tab, waitingMs });

describe("refocusNeeded", () => {
  it.each([
    [true, false, false, true],
    [true, false, true, false],
    [true, true, false, false],
    [true, true, true, false],
    [false, false, false, false],
    [false, false, true, false],
    [false, true, false, false],
    [false, true, true, false],
  ])("rowFocused=%s leaving=%s focusInside=%s → %s", (rowFocused, leaving, focusInside, expected) => {
    expect(refocusNeeded({ rowFocused, leaving, focusInside })).toBe(expected);
  });
});

describe("openerUsable", () => {
  const body = { isConnected: true };
  it.each([
    ["null", null, false],
    ["undefined", undefined, false],
    ["disconnected", { isConnected: false }, false],
    ["body", body, false],
    ["connected", { isConnected: true }, true],
  ] as const)("accepts %s: %s", (_name, opener, expected) => {
    expect(openerUsable(opener, body)).toBe(expected);
  });
});

describe("gateRows", () => {
  it.each([
    { phase: "decided", verdict: "allowed" },
    { phase: "gone", message: "Closed" },
    { phase: "terminal" },
    { phase: "released", afterMs: 60_000 },
  ] satisfies ApprovalRequest["state"][])("ignores $phase requests and keeps pending waits", (state) => {
    const quiet = { ...row("quiet", 1), waitingMs: undefined };
    const pending = { ...row("pending", 2), waitingMs: undefined };
    const result = gateRows([quiet, pending], [
      { ...request("quiet", 180_000), state },
      { ...request("pending", 180_000), state },
      request("pending"),
    ]);
    expect(result[0]).toMatchObject({ status: "idle", waitingMs: undefined });
    expect(result[0]).toBe(quiet);
    expect(result[1]).toMatchObject({ status: "needs-permission", waitingMs: 60_000 });
  });

  it("counts a plain shell with a held request, once per tab, using its longest wait without changing the sidebar", () => {
    const rows = [{ ...row("shell", 1), agent: "cmd" as const }, row("agent", 2), row("quiet", 3)];
    const requests = [request("shell"), request("shell", 120_000), request("agent")];
    const gated = gateRows(rows, requests);
    expect(gated[0]).toMatchObject({ status: "needs-permission", waitingMs: 120_000 });
    expect(splitGroups(boardRows(gated)).gate).toHaveLength(new Set(requests.map((r) => r.tab)).size);
    expect(rows[0]).toMatchObject({ status: "idle" });
    expect(gated[2]).toBe(rows[2]);
  });
  it("keeps a row's question status", () => {
    expect(gateRows([{ ...row("question", 1), status: "needs-question" }], [request("question")])[0]).toMatchObject({ status: "needs-question", waitingMs: 60_000 });
  });
});

describe("heldRows", () => {
  it("keeps missing rows at their last indices and drops no live or new rows", () => {
    const old = [row("a", 1), row("b", 2), row("c", 3), row("d", 4)];
    const kept = new Map(old.map((r) => [r.id, r]));
    const current = { ...old[2]!, name: "Updated" };
    const result = heldRows([old[0]!, current, row("new", 5)], kept, ["b", "d"]);
    expect(result.map((r) => r.id)).toEqual(["a", "b", "c", "d", "new"]);
    expect(result[1]).toBe(old[1]);
    expect(result[2]).toBe(current);
  });
  it("does not duplicate a live row or invent a missing kept row", () => {
    const a = row("a", 1);
    expect(heldRows([a], new Map([["a", a]]), ["a", "unknown"])).toEqual([a]);
  });
});

describe("cardFor", () => {
  it("finds the oldest pending entry, including terminal-only entries", () => {
    const oldest = { ...request("a", 180_000), terminalOnly: true };
    expect(cardFor([request("a"), request("b", 240_000), oldest], "a")).toBe(oldest);
  });
  it("ignores answered entries and tabs without an entry", () => {
    const answered: ApprovalRequest = { ...request("a"), state: { phase: "decided", verdict: "allowed" } };
    expect(cardFor([answered], "a")).toBeUndefined();
    expect(cardFor([answered], "missing")).toBeUndefined();
  });
});

describe("activeIndex", () => {
  it("follows session 4 when another session moves ahead of it", () => {
    expect(activeIndex(["1", "3", "4", "2"], "4", 2)).toBe(2);
    expect(activeIndex(["2", "1", "3", "4"], "4", 2)).toBe(3);
  });
  it("clamps the last index when the id disappears, including an empty board", () => {
    expect(activeIndex(["1", "2", "3"], "4", 1)).toBe(1);
    expect(activeIndex(["1", "2"], "4", 3)).toBe(1);
    expect(activeIndex(["1"], null, -1)).toBe(0);
    expect(activeIndex([], "4", 3)).toBe(0);
  });
});

describe("fitLiveBoard", () => {
  const groups = splitGroups(boardRows(Array.from({ length: 8 }, (_, i) => row(String(i + 1), i + 1))));
  it("keeps a row past the fit as the last whole row without reserving card space", () => {
    const fit = fitLiveBoard(284, groups, undefined, "7");
    expect([...fit.gate, ...fit.others].map((r) => r.id)).toEqual(["4", "5", "6", "7"]);
    expect(fit.hidden).toBe(4);
  });
  it("leaves the fit unchanged when the kept row is already visible", () => {
    expect(fitLiveBoard(284, groups, undefined, "3")).toEqual(fitLiveBoard(284, groups));
  });
  it("gives the card priority over the kept row and reserves its space", () => {
    const fit = fitLiveBoard(504, groups, "6", "8");
    expect([...fit.gate, ...fit.others].map((r) => r.id)).toEqual(["3", "4", "5", "6"]);
    expect(fit.hidden).toBe(4);
    expect(fit).toEqual(fitLiveBoard(504, groups, "6"));
  });
});

describe("focusAfter", () => {
  it.each([
    ["escape", "opener"], ["jump", "terminal"], ["all-sessions", "palette"], ["card-closed", "board-row"],
  ] as const)("sends %s to %s", (action, target) => expect(focusAfter(action)).toBe(target));
});

function board() {
  class Node extends EventTarget {
    isConnected = true;
    constructor(readonly kind: string, readonly id = "") { super(); }
    focus = vi.fn(() => { Object.assign(document, { activeElement: this }); });
    matches = (selector: string) => selector === ".board__row" && this.kind === "row";
    getAttribute = (name: string) => name === "data-row-id" ? this.id : null;
  }
  const opener = new Node("opener");
  vi.stubGlobal("HTMLElement", Node);
  vi.stubGlobal("document", { activeElement: opener });
  const disconnect = vi.fn();
  let resize: (entries: { contentRect: { height: number } }[]) => void = () => {};
  vi.stubGlobal("ResizeObserver", class {
    constructor(callback: typeof resize) { resize = callback; }
    observe = vi.fn();
    disconnect = disconnect;
  });
  const table = new Node("table");
  const root = Object.assign(new Node("root"), {
    contains: (node: unknown) => node === root || node === table || node === all || [...nodes.values()].includes(node as Node),
    getBoundingClientRect: () => ({ height: 800 }),
    querySelector: (selector: string): Node | null => selector === ".board__table" ? table : selector.startsWith(".board__row") ? currentRow : currentRow ?? all,
    querySelectorAll: () => [currentRow, all].filter(Boolean),
  });
  const nodes = new Map<string, Node>();
  const all = new Node("button");
  let currentRow: Node | null = null;
  let rows = [row("1", 1), row("2", 2), row("3", 3), row("4", 4)];
  let requests: ApprovalRequest[] = [];
  let props: Parameters<typeof DeparturesBoard>[0];
  const onJump = vi.fn();
  const onClose = vi.fn();
  const onAllSessions = vi.fn();
  const renderCard = vi.fn(() => "Request");
  const host = mountHook(() => {
    const view = LiveDepartures({ rows, requests, reduced: false, renderCard, onJump, onClose, onAllSessions });
    view.props.ref.current = root;
    props = elements(view).find((e) => e.type === DeparturesBoard)!.props;
    const shown = elements(DeparturesBoard(props)).filter((e) => e.type === "tr" && e.props["data-row-id"]);
    for (const r of shown) {
      const id = r.props["data-row-id"];
      if (!nodes.has(id)) nodes.set(id, new Node("row", id));
    }
    currentRow = nodes.get(shown.find((r) => r.props.tabIndex === 0)?.props["data-row-id"] ?? "") ?? null;
    return view;
  });
  const focus = (node: Node) => {
    node.focus();
    host.current.props.onFocusCapture?.({ target: node });
    host.flush();
  };
  const key = (code: string, target = document.activeElement, shiftKey = false) => {
    const event = { code, key: code === "KeyY" ? "z" : code, target, currentTarget: root, shiftKey, preventDefault: vi.fn(), stopPropagation: vi.fn() };
    host.current.props.onKeyDown(event);
    host.flush();
    return event;
  };
  return { host, nodes, opener, root, table, all, onJump, onClose, onAllSessions, renderCard, disconnect, key, focus,
    get props() { return props; },
    update(nextRows: RowModel[], nextRequests: ApprovalRequest[] = requests) { rows = nextRows; requests = nextRequests; host.rerender(); },
    resize(height: number) { resize([{ contentRect: { height } }]); host.flush(); },
  };
}

describe("LiveDepartures lifecycle", () => {
  it.each(["body", "disconnected"])("asks for terminal focus on Escape when the opener is %s", (kind) => {
    const b = board();
    if (kind === "body") Object.assign(document, { body: b.opener });
    else b.opener.isConnected = false;
    b.key("Escape");
    expect(b.onClose).toHaveBeenCalledExactlyOnceWith(true);
    expect(b.opener.focus).not.toHaveBeenCalled();
    b.host.unmount();
  });

  it("keeps the active session when a new group header pushes it past the fit", () => {
    const b = board();
    const rows = Array.from({ length: 8 }, (_, i) => row(String(i + 1), i + 1));
    b.update(rows);
    b.resize(284);
    b.key("End");
    expect(document.activeElement).toBe(b.nodes.get("4"));
    b.update(rows, [request("1")]);
    expect(document.activeElement).toBe(b.nodes.get("4"));
    b.key("Enter");
    expect(b.onJump).toHaveBeenCalledWith("4");
    b.host.unmount();
  });

  it("ignores clicks and Enter on a stale row without leaving the board", () => {
    const b = board();
    b.focus(b.nodes.get("1")!);
    b.table.dispatchEvent(new Event("pointerenter")); b.host.flush();
    b.update([row("2", 2), row("3", 3), row("4", 4)]);
    const tags = elements(DeparturesBoard(b.props)).filter((e) => e.type === "tr" && e.props["data-row-id"]);
    expect(tags.find((e) => e.props["data-row-id"] === "1")!.props.onClick).toBeUndefined();
    expect(tags.find((e) => e.props["data-row-id"] === "2")!.props.onClick).toBeTypeOf("function");
    b.props.onRowClick!("1");
    b.key("Enter");
    expect(b.onJump).not.toHaveBeenCalled();
    Object.assign(document, { activeElement: null });
    b.host.rerender();
    expect(document.activeElement).toBe(b.nodes.get("1"));
    b.host.unmount();
  });

  it("restores a remounted row when its group changes but its index stays the same", () => {
    const b = board();
    b.focus(b.nodes.get("1")!);
    Object.assign(document, { activeElement: null });
    b.update([row("1", 1), row("2", 2), row("3", 3), row("4", 4)], [request("1")]);
    expect(document.activeElement).toBe(b.nodes.get("1"));
    b.host.unmount();
  });
  it("measures the modal, focuses a row, traps Tab in both directions, and returns Escape to the opener", () => {
    const b = board();
    expect(b.host.current.props).toMatchObject({ role: "dialog", "aria-modal": "true", "aria-label": "Departures" });
    expect(b.props.heightPx).toBe(800);
    expect(document.activeElement).toBe(b.nodes.get("1"));
    b.key("Tab");
    expect(document.activeElement).toBe(b.all);
    b.key("Tab");
    expect(document.activeElement).toBe(b.nodes.get("1"));
    b.key("Tab", document.activeElement, true);
    expect(document.activeElement).toBe(b.all);
    b.key("Escape");
    expect(b.onClose).toHaveBeenCalledExactlyOnceWith(false);
    expect(document.activeElement).toBe(b.opener);
    b.host.unmount();
    expect(b.disconnect).toHaveBeenCalledOnce();
  });
  it("follows the active session through reorder and restores a deleted row's focus to its replacement", () => {
    const b = board();
    b.key("ArrowDown");
    b.key("ArrowDown");
    expect(document.activeElement).toBe(b.nodes.get("3"));
    b.update([row("1", 1), row("2", 2), row("3", 3), row("4", 4)], [request("4")]);
    expect(b.props.active).toBe(3);
    b.focus(b.nodes.get("3")!);
    Object.assign(document, { activeElement: null });
    b.update([row("1", 1), row("2", 2), row("4", 4)]);
    expect(document.activeElement).toBe(b.nodes.get("2"));
    b.key("Enter");
    expect(b.onJump).toHaveBeenCalledWith("2");
    expect(b.opener.focus).not.toHaveBeenCalled();
    b.host.unmount();
  });
  it("holds groups and the latest copy of closed and newly added rows until pointerleave", () => {
    const b = board();
    b.table.dispatchEvent(new Event("pointerenter")); b.host.flush();
    const last = { ...row("2", 2), name: "Last name" };
    b.update([row("1", 1), last, row("3", 3), row("4", 4), row("5", 5)], [request("2")]);
    expect(b.props.hold).toEqual({ order: ["1", "2", "3", "4", "5"], gate: [] });
    b.update([row("1", 1), row("3", 3), row("4", 4)], []);
    expect(b.props.stale).toEqual(["2", "5"]);
    expect(b.props.rows.find((r) => r.id === "2")).toBe(last);
    b.table.dispatchEvent(new Event("pointerleave")); b.host.flush();
    expect(b.props.hold).toBeUndefined();
    expect(b.props.rows.map((r) => r.id)).toEqual(["1", "3", "4"]);
    b.host.unmount();
  });
  it("keeps the bottom row active when its card reduces the number of visible rows", () => {
    const b = board();
    b.update(Array.from({ length: 6 }, (_, i) => row(String(i + 1), i + 1)), Array.from({ length: 6 }, (_, i) => request(String(i + 1))));
    b.resize(440);
    b.key("End");
    b.key("KeyN");
    expect(b.props.card).toBe("6");
    expect(document.activeElement).toBe(b.nodes.get("6"));
    expect(b.renderCard).toHaveBeenLastCalledWith(request("6"), "deny", expect.any(Function));
    b.key("Escape");
    expect(document.activeElement).toBe(b.nodes.get("6"));
    b.host.unmount();
  });
  it("opens physical Y and N with safe focus, closes the card first, and restores answered cards to the board", () => {
    const b = board();
    b.update([row("1", 1)], [request("1")]);
    expect(b.key("KeyY").preventDefault).toHaveBeenCalledOnce();
    b.props.renderCard!("1");
    expect(b.renderCard).toHaveBeenLastCalledWith(request("1"), "title", expect.any(Function));
    b.key("Escape");
    expect(b.props.card).toBeUndefined();
    expect(b.onClose).not.toHaveBeenCalled();
    b.key("KeyN");
    b.props.renderCard!("1");
    expect(b.renderCard).toHaveBeenLastCalledWith(request("1"), "deny", expect.any(Function));
    b.update([row("1", 1)], []);
    expect(b.props.card).toBeUndefined();
    expect(document.activeElement).toBe(b.nodes.get("1"));
    b.props.onAllSessions!();
    expect(b.onAllSessions).toHaveBeenCalledOnce();
    b.host.unmount();
    expect(b.opener.focus).not.toHaveBeenCalled();
  });
});
