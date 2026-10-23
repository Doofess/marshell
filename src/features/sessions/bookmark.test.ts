import { afterEach, describe, expect, it, vi } from "vitest";
import { sessions } from "../../lib/api/sessions";
import { bookmarkSession } from "./bookmark";

const ep = { port: 1, token: "t" } as unknown as Parameters<typeof bookmarkSession>[0];

afterEach(() => vi.restoreAllMocks());

describe("bookmarkSession", () => {
  it("bookmark runs on Ctrl+Shift+B and announces the title", async () => {
    const call = vi.spyOn(sessions, "bookmark").mockResolvedValue({ ok: true, title: "Fix the API" });
    const announce = vi.fn();
    await bookmarkSession(ep, "t1", announce);
    expect(call).toHaveBeenCalledWith(ep, "t1");
    expect(announce).toHaveBeenCalledWith("Bookmarked this point in “Fix the API”.");
  });

  it("names an unnamed conversation instead of leaving the quotes empty", async () => {
    vi.spyOn(sessions, "bookmark").mockResolvedValue({ ok: true, title: "" });
    const announce = vi.fn();
    await bookmarkSession(ep, "t1", announce);
    expect(announce).toHaveBeenCalledWith("Bookmarked this point in “Untitled conversation”.");
  });

  it("bookmark announces the refusal", async () => {
    vi.spyOn(sessions, "bookmark").mockResolvedValue({ ok: false, reason: "Bookmarks work in Claude Code sessions for now." });
    const announce = vi.fn();
    await bookmarkSession(ep, "t1", announce);
    expect(announce).toHaveBeenCalledWith("Bookmarks work in Claude Code sessions for now.");
  });

  it("says why when the request itself fails", async () => {
    vi.spyOn(sessions, "bookmark").mockRejectedValue(new Error("offline"));
    const announce = vi.fn();
    await bookmarkSession(ep, "t1", announce);
    expect(announce).toHaveBeenCalledWith("Couldn't bookmark: offline");
  });
});
