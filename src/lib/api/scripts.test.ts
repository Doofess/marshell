import { afterEach, describe, expect, it, vi } from "vitest";
import type { Endpoint } from "../../generated/Endpoint";
import { ApiError } from "./client";
import { allowScripts, getScripts, runScript } from "./scripts";

const endpoint: Endpoint = { port: 9, token: "t", platform: "windows" };
const respond = (body: unknown, status = 200) => vi.fn().mockResolvedValue(new Response(JSON.stringify(body), { status }));
const call = (fetchMock: ReturnType<typeof vi.fn>) => {
  const [url, init] = fetchMock.mock.calls[0]!;
  return { url: url as string, method: init.method as string, body: init.body ? JSON.parse(init.body) : undefined };
};

afterEach(() => vi.unstubAllGlobals());

describe("scripts api", () => {
  it("reads the scripts of a folder, encoding the path", async () => {
    const read = respond({ root: "C:/a b", trusted: false });
    vi.stubGlobal("fetch", read);
    await getScripts(endpoint, "C:/a b");
    expect(call(read)).toMatchObject({ url: "http://127.0.0.1:9/v1/scripts?cwd=C%3A%2Fa%20b", method: "GET" });
  });

  it("allows a hash and runs a script", async () => {
    const allow = respond({ trusted: true });
    vi.stubGlobal("fetch", allow);
    await allowScripts(endpoint, "C:/a", "h1");
    expect(call(allow)).toMatchObject({ url: "http://127.0.0.1:9/v1/scripts/allow", method: "POST", body: { cwd: "C:/a", hash: "h1" } });

    const run = respond({ tab_id: "t1", number: 2 });
    vi.stubGlobal("fetch", run);
    await expect(runScript(endpoint, { cwd: "C:/a", script: "run", hash: "h1" })).resolves.toEqual({ tab_id: "t1", number: 2 });
    expect(call(run)).toMatchObject({ url: "http://127.0.0.1:9/v1/scripts/run", method: "POST", body: { cwd: "C:/a", script: "run", hash: "h1" } });
  });

  it("rejects a refusal with an ApiError", async () => {
    vi.stubGlobal("fetch", vi.fn().mockResolvedValue(new Response("marshell.json changed. Check the scripts again.", { status: 409 })));
    await expect(allowScripts(endpoint, "C:/a", "old")).rejects.toBeInstanceOf(ApiError);
  });
});
