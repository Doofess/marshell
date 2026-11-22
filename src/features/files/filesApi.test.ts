import { afterEach, describe, expect, it, vi } from "vitest";
import type { Endpoint } from "../../generated/Endpoint";

const ep: Endpoint = { port: 7, token: "t", platform: "windows" };
vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn().mockResolvedValue({ port: 7, token: "t", platform: "windows" }) }));

import { listTree, searchFiles } from "./filesApi";

afterEach(() => vi.unstubAllGlobals());

const reply = { root: "C:/p", dirs: [] };

describe("listTree", () => {
  it("posts the asks as { dirs } and returns the reply", async () => {
    const fetchMock = vi.fn().mockResolvedValue(new Response(JSON.stringify(reply), { status: 200 }));
    vi.stubGlobal("fetch", fetchMock);
    const asks = [{ dir: "", stamp: null }];
    expect(await listTree("a b", asks)).toEqual(reply);
    const [url, init] = fetchMock.mock.calls[0]!;
    expect(url).toBe(`http://127.0.0.1:${ep.port}/v1/sessions/a%20b/tree`);
    expect(init.method).toBe("POST");
    expect(JSON.parse(init.body)).toEqual({ dirs: asks });
  });
  it("answers no-folder on a 404", async () => {
    vi.stubGlobal("fetch", vi.fn().mockResolvedValue(new Response("none", { status: 404 })));
    expect(await listTree("t", [])).toBe("no-folder");
  });
  it("throws on another error", async () => {
    vi.stubGlobal("fetch", vi.fn().mockResolvedValue(new Response("boom", { status: 500 })));
    await expect(listTree("t", [])).rejects.toThrow("boom");
  });
});

describe("searchFiles", () => {
  it("posts { q } and passes the signal", async () => {
    const fetchMock = vi.fn().mockResolvedValue(new Response(JSON.stringify({ root: "r", files: [], cut: null }), { status: 200 }));
    vi.stubGlobal("fetch", fetchMock);
    const ctl = new AbortController();
    await searchFiles("t", "a b", ctl.signal);
    const [url, init] = fetchMock.mock.calls[0]!;
    expect(url).toBe(`http://127.0.0.1:${ep.port}/v1/sessions/t/search`);
    expect(JSON.parse(init.body)).toEqual({ q: "a b" });
    expect(init.signal).toBe(ctl.signal);
  });
  it("answers no-folder on a 404", async () => {
    vi.stubGlobal("fetch", vi.fn().mockResolvedValue(new Response("none", { status: 404 })));
    expect(await searchFiles("t", "ab", new AbortController().signal)).toBe("no-folder");
  });
});
