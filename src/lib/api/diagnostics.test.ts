import { afterEach, describe, expect, it, vi } from "vitest";
import type { Endpoint } from "../../generated/Endpoint";
import { ApiError } from "./client";
import { diagnostics } from "./diagnostics";

const endpoint: Endpoint = { port: 9, token: "t", platform: "windows" };
const respond = (body: BodyInit | null, status = 200) => vi.fn().mockResolvedValue(new Response(body, { status }));
const call = (m: ReturnType<typeof vi.fn>) => {
  const [url, init] = m.mock.calls[0]!;
  return { url: url as string, method: init.method as string, body: init.body ? JSON.parse(init.body) : undefined };
};

afterEach(() => vi.unstubAllGlobals());

describe("diagnostics api", () => {
  it("reads and sets the detailed logging state", async () => {
    const get = respond(JSON.stringify({ available: true, detailed: false }));
    vi.stubGlobal("fetch", get);
    await expect(diagnostics.logging(endpoint)).resolves.toEqual({ available: true, detailed: false });
    expect(call(get)).toMatchObject({ url: "http://127.0.0.1:9/v1/diagnostics/logging", method: "GET" });

    const put = respond(JSON.stringify({ available: true, detailed: true }));
    vi.stubGlobal("fetch", put);
    await diagnostics.setDetailed(endpoint, true);
    expect(call(put)).toMatchObject({ url: "http://127.0.0.1:9/v1/diagnostics/logging", method: "PUT", body: { detailed: true } });
  });

  it("prepares an export with a POST", async () => {
    const post = respond(JSON.stringify({ id: "x", files: [], total_bytes: 0 }));
    vi.stubGlobal("fetch", post);
    await expect(diagnostics.prepare(endpoint)).resolves.toEqual({ id: "x", files: [], total_bytes: 0 });
    expect(call(post)).toMatchObject({ url: "http://127.0.0.1:9/v1/diagnostics/export", method: "POST" });
  });

  it("reads one file as text, encoding the id and the name", async () => {
    const get = respond("hello");
    vi.stubGlobal("fetch", get);
    await expect(diagnostics.file(endpoint, "a/b", "logs/marshell log.txt")).resolves.toBe("hello");
    expect(call(get)).toMatchObject({ url: "http://127.0.0.1:9/v1/diagnostics/export/a%2Fb/file?name=logs%2Fmarshell%20log.txt", method: "GET" });
  });

  it("reads the zip as bytes", async () => {
    const get = respond(new Uint8Array([80, 75, 3, 4]));
    vi.stubGlobal("fetch", get);
    const bytes = await diagnostics.zip(endpoint, "x");
    expect(new Uint8Array(bytes)).toEqual(new Uint8Array([80, 75, 3, 4]));
    expect(call(get)).toMatchObject({ url: "http://127.0.0.1:9/v1/diagnostics/export/x/zip", method: "GET" });
  });

  it("rejects with the status for an expired export", async () => {
    vi.stubGlobal("fetch", respond("This export has expired. Prepare it again.", 404));
    await expect(diagnostics.zip(endpoint, "x")).rejects.toMatchObject({ status: 404 });
    vi.stubGlobal("fetch", respond("gone", 404));
    await expect(diagnostics.file(endpoint, "x", "a")).rejects.toBeInstanceOf(ApiError);
  });
});
