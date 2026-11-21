import { afterEach, describe, expect, it, vi } from "vitest";
import type { Endpoint } from "../../generated/Endpoint";
import { ApiError } from "./client";
import { saveScreenshot } from "./screenshots";

const endpoint: Endpoint = { port: 9, token: "t", platform: "windows" };

afterEach(() => vi.unstubAllGlobals());

describe("saveScreenshot", () => {
  it("posts the PNG with the host and the time, and no file name", async () => {
    const fetchMock = vi
      .fn()
      .mockResolvedValue(new Response(JSON.stringify({ name: "localhost-5173 2026-11-21 134501.png" }), { status: 200 }));
    vi.stubGlobal("fetch", fetchMock);
    const png = new Blob([new Uint8Array([0x89, 0x50])], { type: "image/png" });
    const saved = await saveScreenshot(endpoint, "localhost:5173", "2026-11-21 134501", png);
    expect(saved.name).toBe("localhost-5173 2026-11-21 134501.png");
    const [url, init] = fetchMock.mock.calls[0]!;
    expect(url).toBe("http://127.0.0.1:9/v1/screenshots?host=localhost%3A5173&at=2026-11-21%20134501");
    expect(init.method).toBe("POST");
    expect(init.headers).toMatchObject({ Authorization: "Bearer t", "Content-Type": "image/png" });
    expect(init.body).toBe(png);
  });

  it("rejects with the core's words", async () => {
    vi.stubGlobal("fetch", vi.fn().mockResolvedValue(new Response("That picture is over 64 MB.", { status: 413 })));
    const err = await saveScreenshot(endpoint, "a.dev", "2026-11-21 134501", new Blob([])).catch((e) => e);
    expect(err).toBeInstanceOf(ApiError);
    expect((err as ApiError).status).toBe(413);
    expect((err as ApiError).message).toContain("That picture is over 64 MB.");
  });
});
