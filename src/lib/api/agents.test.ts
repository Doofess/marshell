import { afterEach, describe, expect, it, vi } from "vitest";
import type { AgentSetup } from "../../generated/AgentSetup";
import type { ConfigPreview } from "../../generated/ConfigPreview";
import type { Endpoint } from "../../generated/Endpoint";
import { applyPreview, getSetup, planInstall, planRemove, previewDiff, toDetection } from "./agents";

const endpoint: Endpoint = { port: 9, token: "t", platform: "windows" };
const reply = (body: unknown, status = 200) => vi.fn().mockResolvedValue(new Response(typeof body === "string" ? body : JSON.stringify(body), { status }));
const call = (m: ReturnType<typeof vi.fn>) => {
  const [url, init] = m.mock.calls[0]!;
  return { url: url as string, method: init.method as string, body: init.body ? JSON.parse(init.body) : undefined };
};
const file = (path: string, diff: string, before?: string) => ({ path, scope: "user", applies: "newSessions", before_hash: before, diff, status: "changed" }) as unknown as ConfigPreview["files"][number];
const preview = (files: ConfigPreview["files"]): ConfigPreview => ({ changeset_id: "c1", title: "t", reason: "r", files, stale: false, can_apply: true });
const setup: AgentSetup = {
  agent: "claude",
  found: true,
  version: "2.1.3",
  exe_path: "C:/bin/claude.exe",
  settings_path: "C:/h/.claude/settings.json",
  settings_state: "ok",
  hooks: "none",
  approve_installed: false,
  has_own_statusline: false,
  chained: false,
  bridge_path: "C:/m/bin/marshell-hook.exe",
  snapshots_dir: "C:/m/snapshots",
};

afterEach(() => vi.unstubAllGlobals());

describe("agents api", () => {
  it("reads the setup and plans install and removal", async () => {
    const g = reply(setup);
    vi.stubGlobal("fetch", g);
    await expect(getSetup(endpoint, "claude")).resolves.toEqual(setup);
    expect(call(g)).toMatchObject({ url: "http://127.0.0.1:9/v1/agents/claude/setup", method: "GET" });
    const p = reply(preview([]));
    vi.stubGlobal("fetch", p);
    await planInstall(endpoint, "claude", { status: true, statusline: false, approve: true });
    expect(call(p)).toMatchObject({ url: "http://127.0.0.1:9/v1/agents/claude/hooks/plan", method: "POST", body: { status: true, statusline: false, approve: true } });
    const r = reply({ preview: preview([]), statusline_left: false });
    vi.stubGlobal("fetch", r);
    await planRemove(endpoint, "claude");
    expect(call(r)).toMatchObject({ url: "http://127.0.0.1:9/v1/agents/claude/hooks/remove-plan", method: "POST" });
  });

  it("applyPreview sends each file's before hash", async () => {
    const m = reply({ snapshot_id: "s1", files: [] });
    vi.stubGlobal("fetch", m);
    const res = await applyPreview(endpoint, preview([file("a.json", "d", "h1"), file("b.json", "d")]));
    expect(call(m)).toMatchObject({
      url: "http://127.0.0.1:9/v1/config/apply",
      method: "POST",
      body: { changeset_id: "c1", expected_hashes: [{ path: "a.json", hash: "h1" }, { path: "b.json" }] },
    });
    expect(res).toEqual({ ok: true, result: { snapshot_id: "s1", files: [] } });
  });

  it("applyPreview reports a conflict on 409", async () => {
    vi.stubGlobal("fetch", reply({ reason: "changed", paths: ["a.json"] }, 409));
    await expect(applyPreview(endpoint, preview([file("a.json", "d", "h1")]))).resolves.toEqual({ ok: false, conflict: true });
  });

  it("applyPreview says a refused file was not written, and is no conflict", async () => {
    vi.stubGlobal("fetch", reply({ reason: "refused", paths: ["a.json", "b.json"] }, 409));
    await expect(applyPreview(endpoint, preview([file("a.json", "d", "h1")]))).resolves.toEqual({ ok: false, conflict: false, message: "Marshell can't change a.json, b.json, so nothing was written." });
    vi.stubGlobal("fetch", reply({ reason: "refused", paths: [] }, 409));
    await expect(applyPreview(endpoint, preview([]))).resolves.toEqual({ ok: false, conflict: false, message: "Nothing was written: there is nothing Marshell can change here." });
  });

  it("applyPreview treats each change reason, an unparsable 409 and an expired preview as a conflict", async () => {
    for (const reason of ["changed", "changed outside Marshell", "not what was shown"]) {
      vi.stubGlobal("fetch", reply({ reason, paths: ["a.json"] }, 409));
      await expect(applyPreview(endpoint, preview([]))).resolves.toEqual({ ok: false, conflict: true });
    }
    vi.stubGlobal("fetch", reply("conflict", 409));
    await expect(applyPreview(endpoint, preview([]))).resolves.toEqual({ ok: false, conflict: true });
    vi.stubGlobal("fetch", reply("no such preview", 404));
    await expect(applyPreview(endpoint, preview([]))).resolves.toEqual({ ok: false, conflict: true });
  });

  it("applyPreview passes any other error's message on", async () => {
    vi.stubGlobal("fetch", reply("boom", 500));
    await expect(applyPreview(endpoint, preview([]))).resolves.toEqual({ ok: false, conflict: false, message: "boom" });
  });

  it("toDetection maps unparsable with its line", () => {
    const d = toDetection({ ...setup, settings_state: "unparsable", parse_error: { line: 7, message: "expected value" } });
    expect(d).toMatchObject({ cli: "claude", installed: true, version: "2.1.3", exePath: "C:/bin/claude.exe", configPath: "C:/h/.claude/settings.json", configState: "unparsable", parseError: { line: 7, message: "expected value" }, hasOwnStatusLine: false });
    expect(toDetection({ ...setup, settings_state: "readOnly" }).configState).toBe("read-only");
    expect(toDetection({ ...setup, found: false }).installed).toBe(false);
    expect(toDetection(setup).parseError).toBeUndefined();
  });

  it("previewDiff joins file diffs", () => {
    expect(previewDiff(preview([file("a", "one\n"), file("b", "two\n")]))).toBe("one\ntwo\n");
    expect(previewDiff(preview([file("a", "one"), file("b", ""), file("c", "three")]))).toBe("one\nthree");
  });
});
