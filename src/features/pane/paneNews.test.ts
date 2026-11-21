import { describe, expect, it } from "vitest";
import { newsReducer, panelHasNews } from "./paneNews";

const none: ReadonlySet<string> = new Set();
const item = (session: string, panel: "browser" | "files") => ({ session, panel });

describe("newsReducer", () => {
  it("add_then_seen", () => {
    const added = newsReducer(none, { type: "add", items: [item("a", "files"), item("b", "files")] });
    expect([...added]).toEqual(["files:a", "files:b"]);
    expect([...newsReducer(added, { type: "seen", session: "a", panel: "files" })]).toEqual(["files:b"]);
  });
  it("seen_for_another_session_changes_nothing", () => {
    const s = newsReducer(none, { type: "add", items: [item("a", "files")] });
    expect(newsReducer(s, { type: "seen", session: "b", panel: "files" })).toBe(s);
    expect(newsReducer(s, { type: "add", items: [item("a", "files")] })).toBe(s);
    expect(newsReducer(none, { type: "add", items: [] })).toBe(none);
  });
  it("seen for one panel leaves the other", () => {
    const s = newsReducer(none, { type: "add", items: [item("a", "files"), item("a", "browser")] });
    expect([...newsReducer(s, { type: "seen", session: "a", panel: "files" })]).toEqual(["browser:a"]);
    expect(newsReducer(newsReducer(none, { type: "add", items: [item("a", "files")] }), { type: "seen", session: "a", panel: "browser" }).size).toBe(1);
  });
  it("only the sessions still in the list keep their news, for every panel", () => {
    const s = newsReducer(none, { type: "add", items: [item("a", "files"), item("a", "browser"), item("b", "files"), item("c:d", "browser")] });
    expect([...newsReducer(s, { type: "only", sessions: new Set(["b", "c:d"]) })]).toEqual(["files:b", "browser:c:d"]);
    expect(newsReducer(s, { type: "only", sessions: new Set<string>() }).size).toBe(0);
    // Nothing gone: the same set, so a window that asks on every render doesn't draw again.
    expect(newsReducer(s, { type: "only", sessions: new Set(["a", "b", "c:d", "z"]) })).toBe(s);
  });
  it("a session id with a colon keeps its own keys", () => {
    const s = newsReducer(none, { type: "add", items: [item("a:b", "browser")] });
    expect([...s]).toEqual(["browser:a:b"]);
    expect(newsReducer(s, { type: "only", sessions: new Set(["a:b"]) })).toBe(s);
    expect(newsReducer(s, { type: "only", sessions: new Set(["a", "b"]) }).size).toBe(0);
  });
});

describe("panelHasNews", () => {
  it("is true when any session has news for the panel", () => {
    const s = newsReducer(none, { type: "add", items: [item("a", "files"), item("b:c", "browser")] });
    expect(panelHasNews(s, "files")).toBe(true);
    expect(panelHasNews(s, "browser")).toBe(true);
    expect(panelHasNews(none, "files")).toBe(false);
    expect(panelHasNews(newsReducer(none, { type: "add", items: [item("a", "files")] }), "browser")).toBe(false);
  });
});
