import { afterEach, describe, expect, it, vi } from "vitest";
import type { AgentCatalog } from "../../generated/AgentCatalog";
import type { Endpoint } from "../../generated/Endpoint";
import { ApiError } from "./client";
import { catalogApi } from "./catalog";

const endpoint: Endpoint = { port: 9, token: "t", platform: "windows" };

afterEach(() => vi.unstubAllGlobals());

describe("catalog api", () => {
  it("reads the agent catalog with the bearer token", async () => {
    const body: AgentCatalog = { agents: [], presets: [{ id: "deepseek", name: "DeepSeek", agent: "claude", mark: "deepseek", problem: null }] };
    const fetchMock = vi.fn().mockResolvedValue(new Response(JSON.stringify(body), { status: 200 }));
    vi.stubGlobal("fetch", fetchMock);
    await expect(catalogApi.get(endpoint)).resolves.toEqual(body);
    const [url, init] = fetchMock.mock.calls[0]!;
    expect(url).toBe("http://127.0.0.1:9/v1/agents/catalog");
    expect(init.method).toBe("GET");
    expect(JSON.stringify(init.headers)).toContain("Bearer t");
  });
  it("rejects with an ApiError when the core refuses", async () => {
    vi.stubGlobal("fetch", vi.fn().mockResolvedValue(new Response("no", { status: 500 })));
    await expect(catalogApi.get(endpoint)).rejects.toBeInstanceOf(ApiError);
  });
});
