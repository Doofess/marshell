import { afterEach, describe, expect, it, vi } from "vitest";
import { history } from "../../lib/api/history";
import { actionAnnouncement, deleteConversation, forkConversation } from "./historyActions";

const ep = { port: 1, token: "t" } as unknown as Parameters<typeof forkConversation>[0];
const target = { agent: "claude", session_id: "s1" };

afterEach(() => vi.restoreAllMocks());

describe("forkConversation", () => {
  it("Fork hands the new tab to onResumed", async () => {
    const call = vi.spyOn(history, "fork").mockResolvedValue({ ok: true, tabId: "t9" });
    const onResumed = vi.fn();
    const refuse = vi.fn();
    await forkConversation(ep, target, { cols: 80, rows: 24 }, { onResumed, refuse });
    expect(call).toHaveBeenCalledWith(ep, "claude", "s1", { cols: 80, rows: 24 });
    expect(onResumed).toHaveBeenCalledWith("t9");
    expect(refuse).not.toHaveBeenCalled();
  });

  it("shows the core's reason when it refuses, and a request failure with the reason", async () => {
    const onResumed = vi.fn();
    const refuse = vi.fn();
    vi.spyOn(history, "fork").mockResolvedValueOnce({ ok: false, reason: "This conversation is running in a tab." });
    await forkConversation(ep, target, { cols: 80, rows: 24 }, { onResumed, refuse });
    expect(refuse).toHaveBeenLastCalledWith("This conversation is running in a tab.");
    vi.spyOn(history, "fork").mockRejectedValueOnce(new Error("offline"));
    await forkConversation(ep, target, { cols: 80, rows: 24 }, { onResumed, refuse });
    expect(refuse).toHaveBeenLastCalledWith("Couldn't fork: offline");
    expect(onResumed).not.toHaveBeenCalled();
  });
});

describe("deleteConversation", () => {
  const handlers = () => ({ announce: vi.fn(), closed: vi.fn(), focusList: vi.fn(), refresh: vi.fn(), refuse: vi.fn() });

  it("a delete closes the conversation and refocuses the list", async () => {
    const call = vi.spyOn(history, "remove").mockResolvedValue({ ok: true });
    const h = handlers();
    await deleteConversation(ep, target, "Fix the API", h);
    expect(call).toHaveBeenCalledWith(ep, "claude", "s1");
    expect(h.closed).toHaveBeenCalledTimes(1);
    expect(h.focusList).toHaveBeenCalledTimes(1);
    expect(h.refresh).toHaveBeenCalledTimes(1);
    expect(h.announce).toHaveBeenCalledWith("Deleted Marshell's copy of “Fix the API”.");
    expect(h.refuse).not.toHaveBeenCalled();
  });

  it("keeps the conversation open and gives the core's reason when it refuses", async () => {
    vi.spyOn(history, "remove").mockResolvedValue({
      ok: false,
      reason: "This conversation is running in a tab. Close the tab first, then delete Marshell's copy.",
    });
    const h = handlers();
    await deleteConversation(ep, target, "t", h);
    expect(h.refuse).toHaveBeenCalledWith("This conversation is running in a tab. Close the tab first, then delete Marshell's copy.");
    expect(h.closed).not.toHaveBeenCalled();
    expect(h.focusList).not.toHaveBeenCalled();
    expect(h.announce).not.toHaveBeenCalled();
  });

  it("says why when the request itself fails", async () => {
    vi.spyOn(history, "remove").mockRejectedValue(new Error("offline"));
    const h = handlers();
    await deleteConversation(ep, target, "t", h);
    expect(h.refuse).toHaveBeenCalledWith("Couldn't delete: offline");
    expect(h.closed).not.toHaveBeenCalled();
  });
});

describe("actionAnnouncement", () => {
  const saved = { named: false, title: "Claude's own title" };
  it("says what a change did, in the plan's words", () => {
    expect(actionAnnouncement({ name: "  Billing  " }, { named: true, title: "Billing" })).toBe("Renamed to “Billing”.");
    expect(actionAnnouncement({ name: "   " }, saved)).toBe("Name cleared.");
    expect(actionAnnouncement({ pinned: true }, saved)).toBe("Pinned.");
    expect(actionAnnouncement({ pinned: false }, saved)).toBe("Unpinned.");
    expect(actionAnnouncement({ archived: true }, saved)).toBe("Archived.");
    expect(actionAnnouncement({ archived: false }, saved)).toBe("Unarchived.");
    expect(actionAnnouncement({ tags: ["a"] }, saved)).toBe("Tags saved.");
  });

  it("announces a name the core cleared as cleared, though text was typed", () => {
    expect(actionAnnouncement({ name: "‮" }, saved)).toBe("Name cleared.");
  });

  it("announces the name as saved, not as typed", () => {
    const typed = "x".repeat(300);
    expect(actionAnnouncement({ name: typed }, { named: true, title: "x".repeat(120) })).toBe(`Renamed to “${"x".repeat(120)}”.`);
  });
});
