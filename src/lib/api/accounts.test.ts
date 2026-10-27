import { afterEach, describe, expect, it, vi } from "vitest";
import type { Endpoint } from "../../generated/Endpoint";
import { ApiError } from "./client";
import { getAccount, getInstall, getSettingsView, planSettings, undoSettings } from "./accounts";

const endpoint: Endpoint = { port: 9, token: "t", platform: "windows" };
const reply = (body: unknown) => vi.fn().mockResolvedValue(new Response(JSON.stringify(body), { status: 200 }));
const call = (m: ReturnType<typeof vi.fn>) => {
  const [url, init] = m.mock.calls[0]!;
  return { url: url as string, method: init.method as string };
};

afterEach(() => vi.unstubAllGlobals());

describe("accounts api", () => {
  it("gets the install, with recheck only when asked", async () => {
    const g = reply({ agent: "codex" });
    vi.stubGlobal("fetch", g);
    await getInstall(endpoint, "codex");
    expect(call(g)).toEqual({ url: "http://127.0.0.1:9/v1/agents/codex/install?recheck=false", method: "GET" });
    const r = reply({ agent: "codex" });
    vi.stubGlobal("fetch", r);
    await getInstall(endpoint, "codex", true);
    expect(call(r)).toEqual({ url: "http://127.0.0.1:9/v1/agents/codex/install?recheck=true", method: "GET" });
  });
  it("gets the account status", async () => {
    const g = reply({});
    vi.stubGlobal("fetch", g);
    await getAccount(endpoint, "claude");
    expect(call(g)).toEqual({ url: "http://127.0.0.1:9/v1/agents/claude/account", method: "GET" });
  });
  it("gets the settings view", async () => {
    const g = reply({});
    vi.stubGlobal("fetch", g);
    await getSettingsView(endpoint, "antigravity");
    expect(call(g)).toEqual({ url: "http://127.0.0.1:9/v1/agents/antigravity/settings-view", method: "GET" });
  });
  it("posts a settings plan with its body", async () => {
    const g = reply({});
    vi.stubGlobal("fetch", g);
    await planSettings(endpoint, "codex", { expected_hash: "h", change: { kind: "form", edits: [{ key: "model", value: '"o"' }] } });
    expect(call(g)).toEqual({ url: "http://127.0.0.1:9/v1/agents/codex/settings/plan", method: "POST" });
    expect(JSON.parse(g.mock.calls[0]![1].body)).toEqual({ expected_hash: "h", change: { kind: "form", edits: [{ key: "model", value: '"o"' }] } });
  });
  it("posts an undo with the snapshot id", async () => {
    const g = reply({});
    vi.stubGlobal("fetch", g);
    await undoSettings(endpoint, "claude", "snap1");
    expect(call(g)).toEqual({ url: "http://127.0.0.1:9/v1/agents/claude/settings/undo", method: "POST" });
    expect(JSON.parse(g.mock.calls[0]![1].body)).toEqual({ snapshot_id: "snap1" });
  });
  it("throws the core's sentence on a 422 and the status on a 409", async () => {
    vi.stubGlobal("fetch", vi.fn().mockResolvedValue(new Response("That file does not parse.", { status: 422 })));
    await expect(planSettings(endpoint, "claude", { change: { kind: "raw", text: "x" } })).rejects.toMatchObject({ status: 422, message: "That file does not parse." });
    vi.stubGlobal("fetch", vi.fn().mockResolvedValue(new Response("{}", { status: 409 })));
    const e = await planSettings(endpoint, "claude", { change: { kind: "raw", text: "x" } }).catch((x: unknown) => x);
    expect(e).toBeInstanceOf(ApiError);
    expect((e as ApiError).status).toBe(409);
  });
});
