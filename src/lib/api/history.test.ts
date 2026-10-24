import { afterEach, describe, expect, it, vi } from "vitest";

const invoke = vi.hoisted(() => vi.fn());
vi.mock("@tauri-apps/api/core", () => ({ invoke }));
import type { Endpoint } from "../../generated/Endpoint";
import { ApiError } from "./client";
import { history, saveConversation } from "./history";
import { sessions } from "./sessions";

const endpoint: Endpoint = { port: 9, token: "t", platform: "windows" };
const respond = (body: unknown, status = 200) =>
  vi.fn().mockResolvedValue(new Response(body === null ? null : JSON.stringify(body), { status }));
const call = (fetchMock: ReturnType<typeof vi.fn>) => {
  const [url, init] = fetchMock.mock.calls[0]!;
  return { url: url as string, method: init.method as string, body: init.body ? JSON.parse(init.body) : undefined };
};

afterEach(() => vi.unstubAllGlobals());

describe("history", () => {
  it("lists with only the set fields in the query string", async () => {
    const none = respond([]);
    vi.stubGlobal("fetch", none);
    await history.list(endpoint, {});
    expect(call(none)).toMatchObject({ url: "http://127.0.0.1:9/v1/history", method: "GET" });

    const some = respond([]);
    vi.stubGlobal("fetch", some);
    await history.list(endpoint, { agent: "claude", project: "C:\\dev\\a b", model: undefined, since_ms: 5 });
    expect(call(some).url).toBe("http://127.0.0.1:9/v1/history?agent=claude&project=C%3A%5Cdev%5Ca+b&since_ms=5");
  });

  it("searches with the query encoded", async () => {
    const f = respond([]);
    vi.stubGlobal("fetch", f);
    await history.search(endpoint, "a&b c");
    expect(call(f)).toMatchObject({ url: "http://127.0.0.1:9/v1/history/search?q=a%26b+c", method: "GET" });
  });

  it("gets a conversation, encoding both path segments", async () => {
    const f = respond({ turns: [] });
    vi.stubGlobal("fetch", f);
    await history.get(endpoint, "claude", "a/b");
    expect(call(f)).toMatchObject({ url: "http://127.0.0.1:9/v1/history/claude/a%2Fb", method: "GET" });
  });

  it("resumes with the size, returns the reason on a 409 and rejects on other errors", async () => {
    const ok = respond({ tab_id: "t1", number: 1 });
    vi.stubGlobal("fetch", ok);
    await expect(history.resume(endpoint, "claude", "s 1", { cols: 80, rows: 24 })).resolves.toEqual({ ok: true, tabId: "t1" });
    expect(call(ok)).toMatchObject({
      url: "http://127.0.0.1:9/v1/history/claude/s%201/resume",
      method: "POST",
      body: { cols: 80, rows: 24 },
    });
    vi.stubGlobal("fetch", respond({ reason: "Its folder is gone." }, 409));
    await expect(history.resume(endpoint, "claude", "s", { cols: 80, rows: 24 })).resolves.toEqual({
      ok: false,
      reason: "Its folder is gone.",
    });
    vi.stubGlobal("fetch", vi.fn().mockResolvedValue(new Response("No such conversation.", { status: 404 })));
    await expect(history.resume(endpoint, "claude", "s", { cols: 80, rows: 24 })).rejects.toBeInstanceOf(ApiError);
  });

  it("lists with tag, bookmarked and archived when set, and leaves false booleans out", async () => {
    const f = respond([]);
    vi.stubGlobal("fetch", f);
    await history.list(endpoint, { tag: "to do", bookmarked: false, archived: true });
    expect(call(f).url).toBe("http://127.0.0.1:9/v1/history?tag=to+do&archived=true");
  });

  it("sends place and headless to the list, and headless to the search, only when set", async () => {
    const l = respond([]);
    vi.stubGlobal("fetch", l);
    await history.list(endpoint, { place: "outside", headless: true });
    expect(call(l).url).toBe("http://127.0.0.1:9/v1/history?place=outside&headless=true");
    const f = respond([]);
    vi.stubGlobal("fetch", f);
    await history.search(endpoint, "x", false, true);
    expect(call(f).url).toBe("http://127.0.0.1:9/v1/history/search?q=x&headless=true");
  });

  it("reads the import's status", async () => {
    const f = respond({ on: true, copying: false, done: 0, total: 0 });
    vi.stubGlobal("fetch", f);
    await history.importStatus(endpoint);
    expect(call(f)).toMatchObject({ url: "http://127.0.0.1:9/v1/history/import", method: "GET" });
  });

  it("searches with archived=true only when asked", async () => {
    const f = respond([]);
    vi.stubGlobal("fetch", f);
    await history.search(endpoint, "x", true);
    expect(call(f).url).toBe("http://127.0.0.1:9/v1/history/search?q=x&archived=true");
  });

  it("gets the bookmarks with the filters in the query string", async () => {
    const f = respond([]);
    vi.stubGlobal("fetch", f);
    await history.bookmarks(endpoint, { agent: "claude", bookmarked: true, archived: true });
    expect(call(f)).toMatchObject({
      url: "http://127.0.0.1:9/v1/history/bookmarks?agent=claude&bookmarked=true&archived=true",
      method: "GET",
    });
  });

  it("patches a conversation and returns the entry, or the reason for a 400 or 409", async () => {
    const entry = { session_id: "s", pinned: true };
    const ok = respond(entry);
    vi.stubGlobal("fetch", ok);
    await expect(history.patch(endpoint, "claude", "s 1", { pinned: true })).resolves.toEqual({ ok: true, entry });
    expect(call(ok)).toMatchObject({ url: "http://127.0.0.1:9/v1/history/claude/s%201", method: "PATCH", body: { pinned: true } });
    vi.stubGlobal("fetch", vi.fn().mockResolvedValue(new Response("A tag can't contain a comma.", { status: 400 })));
    await expect(history.patch(endpoint, "claude", "s", { tags: ["a,b"] })).resolves.toEqual({
      ok: false,
      reason: "A tag can't contain a comma.",
    });
    vi.stubGlobal("fetch", vi.fn().mockResolvedValue(new Response("boom", { status: 500 })));
    await expect(history.patch(endpoint, "claude", "s", {})).rejects.toBeInstanceOf(ApiError);
  });

  it("bookmarks a turn with PUT and removes the bookmark with DELETE", async () => {
    const on = respond(null, 204);
    vi.stubGlobal("fetch", on);
    await expect(history.bookmarkTurn(endpoint, "claude", "s", 3, true)).resolves.toEqual({ ok: true });
    expect(call(on)).toMatchObject({ url: "http://127.0.0.1:9/v1/history/claude/s/bookmarks/3", method: "PUT" });
    const off = respond(null, 204);
    vi.stubGlobal("fetch", off);
    await history.bookmarkTurn(endpoint, "claude", "s", 3, false);
    expect(call(off)).toMatchObject({ method: "DELETE" });
  });

  it("hands back the core's words when it has not indexed the turn (409)", async () => {
    const text = "Marshell hasn't indexed turn 3 of this conversation yet.";
    vi.stubGlobal("fetch", vi.fn().mockResolvedValue(new Response(text, { status: 409 })));
    await expect(history.bookmarkTurn(endpoint, "claude", "s", 3, true)).resolves.toEqual({ ok: false, reason: text });
    vi.stubGlobal("fetch", vi.fn().mockResolvedValue(new Response("boom", { status: 500 })));
    await expect(history.bookmarkTurn(endpoint, "claude", "s", 3, true)).rejects.toBeInstanceOf(ApiError);
  });

  it("removes Marshell's copy with DELETE and returns the reason for a 409", async () => {
    const f = respond(null, 204);
    vi.stubGlobal("fetch", f);
    await expect(history.remove(endpoint, "claude", "s")).resolves.toEqual({ ok: true });
    expect(call(f)).toMatchObject({ url: "http://127.0.0.1:9/v1/history/claude/s", method: "DELETE" });
    vi.stubGlobal("fetch", respond({ reason: "Running." }, 409));
    await expect(history.remove(endpoint, "claude", "s")).resolves.toEqual({ ok: false, reason: "Running." });
  });

  it("forks with the size", async () => {
    const f = respond({ tab_id: "t2", number: 2 });
    vi.stubGlobal("fetch", f);
    await expect(history.fork(endpoint, "claude", "s", { cols: 80, rows: 24 })).resolves.toEqual({ ok: true, tabId: "t2" });
    expect(call(f)).toMatchObject({ url: "http://127.0.0.1:9/v1/history/claude/s/fork", method: "POST", body: { cols: 80, rows: 24 } });
  });

  it("exports as text with the format and redact in the query string", async () => {
    const f = vi.fn().mockResolvedValue(new Response("# Title", { status: 200 }));
    vi.stubGlobal("fetch", f);
    await expect(history.exportText(endpoint, "claude", "s", "json", false)).resolves.toBe("# Title");
    expect(call(f)).toMatchObject({ url: "http://127.0.0.1:9/v1/history/claude/s/export?format=json&redact=false", method: "GET" });
    const cast = vi.fn().mockResolvedValue(new Response('{"version":2}\n', { status: 200 }));
    vi.stubGlobal("fetch", cast);
    await history.exportText(endpoint, "claude", "s", "asciicast", true);
    expect(call(cast).url).toBe("http://127.0.0.1:9/v1/history/claude/s/export?format=asciicast&redact=true");
    vi.stubGlobal("fetch", vi.fn().mockResolvedValue(new Response("No such conversation.", { status: 404 })));
    await expect(history.exportText(endpoint, "claude", "s", "markdown", true)).rejects.toBeInstanceOf(ApiError);
  });

  it("saves through the shell with the file name in a header", async () => {
    invoke.mockResolvedValue("saved/a-2026-10-23.md");
    await expect(saveConversation("héllo", "a-2026-10-23.md")).resolves.toBe("saved/a-2026-10-23.md");
    const [command, bytes, options] = invoke.mock.calls[0]!;
    expect(command).toBe("save_conversation");
    expect(new TextDecoder().decode(bytes as Uint8Array)).toBe("héllo");
    expect(options).toEqual({ headers: { "x-file-name": "a-2026-10-23.md" } });
  });

  it("bookmarks a live session's current point, and returns the reason for a 409", async () => {
    const f = respond({ title: "Fix it" });
    vi.stubGlobal("fetch", f);
    await expect(sessions.bookmark(endpoint, "t 1")).resolves.toEqual({ ok: true, title: "Fix it" });
    expect(call(f)).toMatchObject({ url: "http://127.0.0.1:9/v1/sessions/t%201/bookmark", method: "POST" });
    vi.stubGlobal("fetch", vi.fn().mockResolvedValue(new Response("Bookmarks work in Claude Code sessions for now.", { status: 409 })));
    await expect(sessions.bookmark(endpoint, "t")).resolves.toEqual({
      ok: false,
      reason: "Bookmarks work in Claude Code sessions for now.",
    });
  });
});

describe("history recordings", () => {
  it("lists the recording parts of a conversation", async () => {
    const f = respond([]);
    vi.stubGlobal("fetch", f);
    await history.recordingParts(endpoint, "claude", "a/b");
    expect(call(f)).toMatchObject({ url: "http://127.0.0.1:9/v1/history/claude/a%2Fb/recordings", method: "GET" });
  });

  it("asks for frames with only the set query fields, zero included", async () => {
    const none = respond({});
    vi.stubGlobal("fetch", none);
    await history.recordingFrames(endpoint, "claude", "s", 2, {});
    expect(call(none).url).toBe("http://127.0.0.1:9/v1/history/claude/s/recordings/2/frames");

    const some = respond({});
    vi.stubGlobal("fetch", some);
    await history.recordingFrames(endpoint, "claude", "s", 0, { fromMs: 0, toMs: 5000, offset: 123 });
    expect(call(some).url).toBe("http://127.0.0.1:9/v1/history/claude/s/recordings/0/frames?from_ms=0&to_ms=5000&offset=123");
  });
});
