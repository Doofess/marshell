import { describe, expect, it } from "vitest";
import type { HistoryEntry } from "../../generated/HistoryEntry";
import type { RowModel } from "../sidebar/types";
import { SCENARIO_ROWS } from "../window/scenario";
import { ApiError } from "../../lib/api/client";
import { NO_ARCHIVE_COPY } from "./ConversationView";
import { EMPTY_LIST_COPY } from "./HistoryView";
import { NO_FILTER_MATCH_COPY, conversationFailure, failureText, listEmptyText, liveTabFor } from "./LiveHistory";

const entry = (tab_id: string): HistoryEntry => ({
  agent: "claude",
  session_id: "s1",
  tab_id,
  cwd: "C:\dev\a",
  title: "t",
  limited: false,
  tags: [],
  pinned: false,
  archived: false,
  bookmarks: 0,
  named: false,
  turns: 1,
  started_ms: 0,
  updated_ms: 0,
});
const F = { when: "any", bookmarked: false, archived: false } as const;
const row = (id: string, status: RowModel["status"]): RowModel => ({ ...SCENARIO_ROWS[0]!, id, status });

describe("liveTabFor", () => {
  it("gives the tab of a row that is not Ended", () => {
    const f = liveTabFor([row("t1", "working"), row("t2", "idle")]);
    expect(f(entry("t1"))).toBe("t1");
    expect(f(entry("t2"))).toBe("t2");
  });
  it("gives null for an Ended row and for a tab no row holds", () => {
    const f = liveTabFor([row("t1", "ended")]);
    expect(f(entry("t1"))).toBeNull();
    expect(f(entry("gone"))).toBeNull();
  });
});

describe("listEmptyText", () => {
  it("says loading until the list has loaded, and the error when it failed", () => {
    expect(listEmptyText(F, false, null)).toBe("Loading conversations…");
    expect(listEmptyText(F, true, "disk full")).toBe("disk full");
  });
  it("says there is nothing yet only when no filter is set", () => {
    expect(listEmptyText(F, true, null)).toBe(EMPTY_LIST_COPY);
  });
  it("names the shortcut when Bookmarked is on and nothing is bookmarked", () => {
    expect(listEmptyText({ ...F, bookmarked: true }, true, null, "Ctrl+Shift+B")).toBe(
      "No bookmarks yet. Press Ctrl+Shift+B in a Claude Code session, or bookmark a turn here.",
    );
  });
  it("says nothing matches when a filter or a time range is set", () => {
    expect(listEmptyText({ ...F, when: "today" }, true, null)).toBe(NO_FILTER_MATCH_COPY);
    expect(listEmptyText({ ...F, project: "x" }, true, null)).toBe(NO_FILTER_MATCH_COPY);
    expect(NO_FILTER_MATCH_COPY).toBe("No conversations match these filters.");
  });
});

describe("failure text", () => {
  it("says the core's words, or the error's, and never an empty message", () => {
    expect(failureText(new ApiError(500, "disk full"))).toBe("disk full");
    expect(failureText(new ApiError(503, " "))).toBe("The core answered 503.");
    expect(failureText(new Error("offline"))).toBe("offline");
  });
  it("shows the view's own copy for a 404 that says there is no archive, and the core's words for anything else", () => {
    expect(conversationFailure(new ApiError(404, "Marshell has no copy of this conversation yet."))).toBe(NO_ARCHIVE_COPY);
    expect(conversationFailure(new ApiError(404, "No such conversation."))).toBe("No such conversation.");
    expect(conversationFailure(new ApiError(500, "boom"))).toBe("boom");
  });
});
