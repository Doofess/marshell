import { afterEach, describe, expect, it, vi } from "vitest";
import type { Endpoint } from "../../generated/Endpoint";
import { ApiError } from "./client";
import { notify } from "./notify";

const endpoint: Endpoint = { port: 9, token: "t", platform: "windows" };

afterEach(() => vi.unstubAllGlobals());

describe("notify api", () => {
  it("reads the paused flag", async () => {
    const fetchMock = vi.fn().mockResolvedValue(new Response(JSON.stringify({ paused: true }), { status: 200 }));
    vi.stubGlobal("fetch", fetchMock);
    expect(await notify.paused(endpoint)).toBe(true);
    const [url, init] = fetchMock.mock.calls[0]!;
    expect(url).toBe("http://127.0.0.1:9/v1/notify/paused");
    expect(init.method).toBe("GET");
  });
  it("sets it with a PUT and an empty answer", async () => {
    const fetchMock = vi.fn().mockResolvedValue(new Response(null, { status: 204 }));
    vi.stubGlobal("fetch", fetchMock);
    await notify.setPaused(endpoint, true);
    const [url, init] = fetchMock.mock.calls[0]!;
    expect(url).toBe("http://127.0.0.1:9/v1/notify/paused");
    expect(init.method).toBe("PUT");
    expect(JSON.parse(init.body)).toEqual({ paused: true });
  });
  it("rejects when the core refuses", async () => {
    vi.stubGlobal("fetch", vi.fn().mockResolvedValue(new Response("no", { status: 401 })));
    await expect(notify.setPaused(endpoint, false)).rejects.toBeInstanceOf(ApiError);
  });
});
