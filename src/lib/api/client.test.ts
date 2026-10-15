import { afterEach, describe, expect, it, vi } from "vitest";
import type { Endpoint } from "../../generated/Endpoint";
import { ApiError, api, getEndpoint, resetEndpoint } from "./client";

const invoke = vi.hoisted(() => vi.fn());
vi.mock("@tauri-apps/api/core", () => ({ invoke }));

const endpoint: Endpoint = { port: 4321, token: "secret", platform: "windows" };

afterEach(() => {
  vi.unstubAllGlobals();
  invoke.mockReset();
  resetEndpoint();
});

describe("api", () => {
  it("sends bearer token and JSON", async () => {
    const fetchMock = vi.fn().mockResolvedValue(new Response(JSON.stringify({ ok: 1 }), { status: 200 }));
    vi.stubGlobal("fetch", fetchMock);

    const out = await api<{ ok: number }>(endpoint, "POST", "/v1/sessions", { cols: 80, rows: 24 });

    expect(out).toEqual({ ok: 1 });
    const [url, init] = fetchMock.mock.calls[0]!;
    expect(url).toBe("http://127.0.0.1:4321/v1/sessions");
    expect(init.method).toBe("POST");
    expect(init.headers.Authorization).toBe("Bearer secret");
    expect(init.headers["Content-Type"]).toBe("application/json");
    expect(JSON.parse(init.body)).toEqual({ cols: 80, rows: 24 });
  });

  it("sends no body or content type without a body", async () => {
    const fetchMock = vi.fn().mockResolvedValue(new Response("[]", { status: 200 }));
    vi.stubGlobal("fetch", fetchMock);

    await api(endpoint, "GET", "/v1/projects");

    const [, init] = fetchMock.mock.calls[0]!;
    expect(init.body).toBeUndefined();
    expect(init.headers["Content-Type"]).toBeUndefined();
  });

  it("throws ApiError with response text on 400", async () => {
    vi.stubGlobal("fetch", vi.fn().mockResolvedValue(new Response("name is empty", { status: 400 })));

    const err = await api(endpoint, "PUT", "/v1/profiles", {}).catch((e: unknown) => e);

    expect(err).toBeInstanceOf(ApiError);
    expect((err as ApiError).status).toBe(400);
    expect((err as ApiError).message).toBe("name is empty");
  });

  it("returns undefined on 204", async () => {
    vi.stubGlobal("fetch", vi.fn().mockResolvedValue(new Response(null, { status: 204 })));

    await expect(api(endpoint, "DELETE", "/v1/sessions/t1")).resolves.toBeUndefined();
  });
});

describe("getEndpoint", () => {
  it("asks the shell once, until reset", async () => {
    invoke.mockResolvedValue(endpoint);

    await expect(getEndpoint()).resolves.toEqual(endpoint);
    await getEndpoint();
    expect(invoke).toHaveBeenCalledTimes(1);
    expect(invoke).toHaveBeenCalledWith("core_endpoint");

    resetEndpoint();
    await getEndpoint();
    expect(invoke).toHaveBeenCalledTimes(2);
  });

  it("does not cache a failed lookup", async () => {
    invoke.mockRejectedValueOnce("no core").mockResolvedValueOnce(endpoint);

    await expect(getEndpoint()).rejects.toBe("no core");
    await expect(getEndpoint()).resolves.toEqual(endpoint);
  });

  it("a stale failure does not drop a newer lookup", async () => {
    let failFirst: (e: unknown) => void = () => {};
    invoke.mockReturnValueOnce(new Promise((_, reject) => (failFirst = reject)));
    const first = getEndpoint().catch(() => undefined);

    resetEndpoint();
    invoke.mockResolvedValueOnce(endpoint);
    await getEndpoint();
    failFirst("late");
    await first;
    await getEndpoint();

    expect(invoke).toHaveBeenCalledTimes(2);
  });
});
