import { afterEach, describe, expect, it, vi } from "vitest";
import type { Endpoint } from "../../generated/Endpoint";
import { getAccount, getInstall, getSettingsView } from "./accounts";

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
});
