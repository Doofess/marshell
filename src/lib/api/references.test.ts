import { afterEach, describe, expect, it, vi } from "vitest";
import type { Endpoint } from "../../generated/Endpoint";
import { ApiError } from "./client";
import { getReferences, putReferences } from "./references";

const endpoint: Endpoint = { port: 9, token: "t", platform: "windows" };
const respond = (body: unknown, status = 200) => vi.fn().mockResolvedValue(new Response(JSON.stringify(body), { status }));
const call = (fetchMock: ReturnType<typeof vi.fn>) => {
  const [url, init] = fetchMock.mock.calls[0]!;
  return { url: url as string, method: init.method as string, body: init.body ? JSON.parse(init.body) : undefined };
};

afterEach(() => vi.unstubAllGlobals());

describe("references api", () => {
  it("reads a project's references, encoding the path", async () => {
    const read = respond({ root: "C:/a b", enabled: false, aliases: [] });
    vi.stubGlobal("fetch", read);
    await expect(getReferences(endpoint, "C:/a b")).resolves.toEqual({ root: "C:/a b", enabled: false, aliases: [] });
    expect(call(read)).toMatchObject({ url: "http://127.0.0.1:9/v1/references?cwd=C%3A%2Fa%20b", method: "GET" });
  });

  it("saves the toggle and aliases with a PUT", async () => {
    const put = vi.fn().mockResolvedValue(new Response(null, { status: 204 }));
    vi.stubGlobal("fetch", put);
    const update = { root: "C:/a", enabled: true, aliases: [{ name: "plan", path: "docs/plan.md" }] };
    await expect(putReferences(endpoint, update)).resolves.toBeUndefined();
    expect(call(put)).toMatchObject({ url: "http://127.0.0.1:9/v1/references", method: "PUT", body: update });
  });

  it("rejects a refusal with an ApiError carrying the core's sentence", async () => {
    vi.stubGlobal("fetch", vi.fn().mockImplementation(async () => new Response("\"../x\" isn't inside the project.", { status: 422 })));
    const update = { root: "C:/a", enabled: true, aliases: [{ name: "x", path: "../x" }] };
    await expect(putReferences(endpoint, update)).rejects.toBeInstanceOf(ApiError);
    await expect(putReferences(endpoint, update)).rejects.toMatchObject({ status: 422, message: "\"../x\" isn't inside the project." });
  });
});
