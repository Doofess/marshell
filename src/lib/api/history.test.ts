import { afterEach, describe, expect, it, vi } from "vitest";
import type { Endpoint } from "../../generated/Endpoint";
import { ApiError } from "./client";
import { history } from "./history";

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
    await expect(history.resume(endpoint, "claude", "s 1", { cols: 80, rows: 24 })).resolves.toEqual({ ok: true });
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
});
