import { afterEach, describe, expect, it, vi } from "vitest";
import type { ConfigPreview } from "../../generated/ConfigPreview";
import type { Endpoint } from "../../generated/Endpoint";
import { ApiError } from "./client";
import { applyFix, doctorReport, lastFix, planFix, runDoctor, undoDifferences, undoFix } from "./doctor";

const endpoint: Endpoint = { port: 9, token: "t", platform: "windows" };
const reply = (body: unknown, status = 200) => vi.fn().mockImplementation(async () => new Response(status === 204 ? null : typeof body === "string" ? body : JSON.stringify(body), { status }));
const call = (m: ReturnType<typeof vi.fn>) => {
  const [url, init] = m.mock.calls[0]!;
  return { url: url as string, method: init.method as string, body: init.body ? JSON.parse(init.body) : undefined };
};
const preview: ConfigPreview = {
  changeset_id: "c1",
  title: "t",
  reason: "r",
  files: [
    { path: "C:/a.json", scope: "user", applies: "newSessions", before_hash: "h1", diff: "d", status: { kind: "changes" } },
    { path: "C:/b.json", scope: "user", applies: "newSessions", diff: "d", status: { kind: "changes" } },
  ],
  stale: false,
  can_apply: true,
} as unknown as ConfigPreview;
const last = { snapshot_id: "s1", fixed_at_ms: 5, item_ids: ["hooks.claude"], count: 1, restart: ["tab-1"], files_match: true };

afterEach(() => vi.unstubAllGlobals());

describe("doctor api", () => {
  it("runDoctor posts to /v1/doctor/run and returns the summary", async () => {
    const f = reply({ running: true, issues: 0 }, 202);
    vi.stubGlobal("fetch", f);
    expect(await runDoctor(endpoint)).toEqual({ running: true, issues: 0 });
    expect(call(f)).toMatchObject({ url: "http://127.0.0.1:9/v1/doctor/run", method: "POST", body: undefined });
  });
  it("doctorReport gets the report, or null on a 204", async () => {
    const f = reply({ items: [], checked_at_ms: 1 });
    vi.stubGlobal("fetch", f);
    expect(await doctorReport(endpoint)).toEqual({ items: [], checked_at_ms: 1 });
    expect(call(f)).toMatchObject({ url: "http://127.0.0.1:9/v1/doctor/report", method: "GET" });
    vi.stubGlobal("fetch", reply(null, 204));
    expect(await doctorReport(endpoint)).toBeNull();
  });
  it("planFix posts the ids, and a 422 rejects with its message", async () => {
    const f = reply({ preview, item_ids: ["a", "b"], restart: [] });
    vi.stubGlobal("fetch", f);
    await planFix(endpoint, ["a", "b"]);
    expect(call(f)).toMatchObject({ url: "http://127.0.0.1:9/v1/doctor/fix", method: "POST", body: { item_ids: ["a", "b"] } });
    vi.stubGlobal("fetch", reply("That item can't be fixed any more.", 422));
    await expect(planFix(endpoint, ["a"])).rejects.toMatchObject({ status: 422, message: "That item can't be fixed any more." });
    await expect(planFix(endpoint, ["a"])).rejects.toBeInstanceOf(ApiError);
  });
  it("applyFix sends the changeset with the hash each file had", async () => {
    const f = reply(last);
    vi.stubGlobal("fetch", f);
    expect(await applyFix(endpoint, preview)).toEqual({ ok: true, fix: last });
    expect(call(f)).toMatchObject({
      url: "http://127.0.0.1:9/v1/doctor/fix/apply",
      method: "POST",
      body: { changeset_id: "c1", expected_hashes: [{ path: "C:/a.json", hash: "h1" }, { path: "C:/b.json" }] },
    });
  });
  it("applyFix maps 404 and a changed 409 to a conflict, and a refusal to a message", async () => {
    vi.stubGlobal("fetch", reply("gone", 404));
    expect(await applyFix(endpoint, preview)).toEqual({ ok: false, conflict: true });
    vi.stubGlobal("fetch", reply({ reason: "changed", paths: ["C:/a.json"] }, 409));
    expect(await applyFix(endpoint, preview)).toEqual({ ok: false, conflict: true });
    vi.stubGlobal("fetch", reply({ reason: "refused", paths: ["C:/a.json"] }, 409));
    expect(await applyFix(endpoint, preview)).toEqual({ ok: false, conflict: false, message: "Marshell can't change C:/a.json, so nothing was written." });
    vi.stubGlobal("fetch", reply("boom", 500));
    expect(await applyFix(endpoint, preview)).toEqual({ ok: false, conflict: false, message: "boom" });
  });
  it("lastFix gets /v1/doctor/undo, or null on a 204", async () => {
    const f = reply(last);
    vi.stubGlobal("fetch", f);
    expect(await lastFix(endpoint)).toEqual(last);
    expect(call(f)).toMatchObject({ url: "http://127.0.0.1:9/v1/doctor/undo", method: "GET" });
    vi.stubGlobal("fetch", reply(null, 204));
    expect(await lastFix(endpoint)).toBeNull();
  });
  it("undoFix posts to /v1/doctor/undo and reads a changed 409 as the files that changed", async () => {
    const f = reply({ snapshot_id: "s1", restored: [] });
    vi.stubGlobal("fetch", f);
    expect(await undoFix(endpoint)).toEqual({ ok: true });
    expect(call(f)).toMatchObject({ url: "http://127.0.0.1:9/v1/doctor/undo", method: "POST" });
    vi.stubGlobal("fetch", reply({ reason: "changed", paths: ["C:/a.json"] }, 409));
    expect(await undoFix(endpoint)).toEqual({ ok: false, changed: ["C:/a.json"] });
    vi.stubGlobal("fetch", reply(null, 204));
    expect(await undoFix(endpoint)).toEqual({ ok: false, message: "There is nothing to undo." });
    vi.stubGlobal("fetch", reply("boom", 500));
    expect(await undoFix(endpoint)).toEqual({ ok: false, message: "boom" });
  });
  it("undoDifferences posts to the snapshot's restore route, which only previews", async () => {
    const f = reply(preview);
    vi.stubGlobal("fetch", f);
    expect(await undoDifferences(endpoint, "s 1")).toEqual(preview);
    expect(call(f)).toMatchObject({ url: "http://127.0.0.1:9/v1/snapshots/s%201/restore", method: "POST" });
  });
});
