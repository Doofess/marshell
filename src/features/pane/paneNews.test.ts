import { describe, expect, it } from "vitest";
import { newsReducer } from "./paneNews";

const none: ReadonlySet<string> = new Set();

describe("newsReducer", () => {
  it("add_then_seen", () => {
    const added = newsReducer(none, { type: "add", sessions: ["a", "b"] });
    expect([...added]).toEqual(["a", "b"]);
    expect([...newsReducer(added, { type: "seen", session: "a" })]).toEqual(["b"]);
  });
  it("seen_for_another_session_changes_nothing", () => {
    const s = newsReducer(none, { type: "add", sessions: ["a"] });
    expect(newsReducer(s, { type: "seen", session: "b" })).toBe(s);
    expect(newsReducer(s, { type: "add", sessions: ["a"] })).toBe(s);
    expect(newsReducer(none, { type: "add", sessions: [] })).toBe(none);
  });
  it("closed_forgets_the_session", () => {
    const s = newsReducer(none, { type: "add", sessions: ["a"] });
    expect(newsReducer(s, { type: "closed", session: "a" }).size).toBe(0);
    expect(newsReducer(s, { type: "closed", session: "x" })).toBe(s);
  });
});
