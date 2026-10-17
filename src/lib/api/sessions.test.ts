import { afterEach, describe, expect, it, vi } from "vitest";
import type { Endpoint } from "../../generated/Endpoint";
import { ApiError } from "./client";
import { profiles } from "./profiles";
import { projects } from "./projects";
import { sessions } from "./sessions";

const endpoint: Endpoint = { port: 9, token: "t", platform: "windows" };
const respond = (body: unknown, status = 200) =>
  vi.fn().mockResolvedValue(new Response(body === null ? null : JSON.stringify(body), { status }));
const call = (fetchMock: ReturnType<typeof vi.fn>) => {
  const [url, init] = fetchMock.mock.calls[0]!;
  return { url: url as string, method: init.method as string, body: init.body ? JSON.parse(init.body) : undefined };
};

afterEach(() => vi.unstubAllGlobals());

describe("sessions", () => {
  it("lists and creates", async () => {
    const list = respond({ sessions: [], closed: [] });
    vi.stubGlobal("fetch", list);
    await sessions.list(endpoint);
    expect(call(list)).toMatchObject({ url: "http://127.0.0.1:9/v1/sessions", method: "GET" });

    const create = respond({ tab_id: "t1", number: 1 });
    vi.stubGlobal("fetch", create);
    await expect(sessions.create(endpoint, { cols: 80, rows: 24 })).resolves.toEqual({ tab_id: "t1", number: 1 });
    expect(call(create)).toMatchObject({
      url: "http://127.0.0.1:9/v1/sessions",
      method: "POST",
      body: { cols: 80, rows: 24 },
    });
  });

  it("renames, reorders and closes, encoding the tab id", async () => {
    const rename = respond(null, 204);
    vi.stubGlobal("fetch", rename);
    await sessions.rename(endpoint, "a/b", "New");
    expect(call(rename)).toMatchObject({
      url: "http://127.0.0.1:9/v1/sessions/a%2Fb",
      method: "PATCH",
      body: { name: "New" },
    });

    const reorder = respond(null, 204);
    vi.stubGlobal("fetch", reorder);
    await sessions.reorder(endpoint, ["b", "a"]);
    expect(call(reorder)).toMatchObject({
      url: "http://127.0.0.1:9/v1/sessions/reorder",
      method: "POST",
      body: { tab_ids: ["b", "a"] },
    });

    const close = respond(null, 204);
    vi.stubGlobal("fetch", close);
    await sessions.close(endpoint, "t1");
    expect(call(close)).toMatchObject({ url: "http://127.0.0.1:9/v1/sessions/t1", method: "DELETE" });
  });

  it("reopens the latest, or a given tab, and surfaces 404", async () => {
    const latest = respond({ tab_id: "t2", number: 2 });
    vi.stubGlobal("fetch", latest);
    await sessions.reopen(endpoint);
    expect(call(latest)).toMatchObject({ url: "http://127.0.0.1:9/v1/sessions/reopen", method: "POST" });
    expect(call(latest).body).toBeUndefined();

    const given = respond({ tab_id: "t3", number: 3 });
    vi.stubGlobal("fetch", given);
    await sessions.reopen(endpoint, "t3");
    expect(call(given).body).toEqual({ tab_id: "t3" });

    vi.stubGlobal("fetch", vi.fn().mockResolvedValue(new Response("nothing to reopen", { status: 404 })));
    const err = await sessions.reopen(endpoint).catch((e: unknown) => e);
    expect(err).toBeInstanceOf(ApiError);
    expect((err as ApiError).status).toBe(404);
  });
});

describe("sessions.input", () => {
  it("posts the text to the tab's input route, encoding the id, and adds no Enter", async () => {
    const input = respond(null, 204);
    vi.stubGlobal("fetch", input);
    await sessions.input(endpoint, "a/b", "Merging x into main conflicts in a.txt.");
    expect(call(input)).toMatchObject({ url: "http://127.0.0.1:9/v1/sessions/a%2Fb/input", method: "POST", body: { text: "Merging x into main conflicts in a.txt." } });
  });
  it("rejects with the core's status when the tab is unknown or the text has a line break", async () => {
    vi.stubGlobal("fetch", respond(null, 404));
    await expect(sessions.input(endpoint, "t1", "x")).rejects.toMatchObject({ status: 404 });
    vi.stubGlobal("fetch", vi.fn().mockResolvedValue(new Response("The text can't contain a line break.", { status: 400 })));
    await expect(sessions.input(endpoint, "t1", "a\nb")).rejects.toBeInstanceOf(ApiError);
  });
});

describe("sessions.resume", () => {
  it("posts the size and resolves ok", async () => {
    const ok = respond({ tab_id: "t 1", number: 1 });
    vi.stubGlobal("fetch", ok);
    await expect(sessions.resume(endpoint, "t 1", { cols: 90, rows: 30 })).resolves.toEqual({ ok: true });
    expect(call(ok)).toMatchObject({
      url: "http://127.0.0.1:9/v1/sessions/t%201/resume",
      method: "POST",
      body: { cols: 90, rows: 30 },
    });
  });

  it("returns the core's reason on a 409 and rejects on other errors", async () => {
    vi.stubGlobal("fetch", respond({ reason: "Its shell profile was removed." }, 409));
    await expect(sessions.resume(endpoint, "t1", { cols: 80, rows: 24 })).resolves.toEqual({
      ok: false,
      reason: "Its shell profile was removed.",
    });
    vi.stubGlobal("fetch", vi.fn().mockResolvedValue(new Response("No such session.", { status: 404 })));
    await expect(sessions.resume(endpoint, "t1", { cols: 80, rows: 24 })).rejects.toBeInstanceOf(ApiError);
  });
});

describe("sessions.restart", () => {
  it("posts the mode and size and resolves ok", async () => {
    const ok = respond({ tab_id: "t 1", number: 1 });
    vi.stubGlobal("fetch", ok);
    await expect(sessions.restart(endpoint, "t 1", "bypass", { cols: 90, rows: 30 })).resolves.toEqual({ ok: true });
    expect(call(ok)).toMatchObject({ url: "http://127.0.0.1:9/v1/sessions/t%201/restart", method: "POST", body: { mode: "bypass", cols: 90, rows: 30 } });
  });
  it("returns the core's reason on a 409 and rejects on other errors", async () => {
    vi.stubGlobal("fetch", respond({ reason: "Only an agent session has a mode." }, 409));
    await expect(sessions.restart(endpoint, "t1", "plan", { cols: 80, rows: 24 })).resolves.toEqual({ ok: false, reason: "Only an agent session has a mode." });
    vi.stubGlobal("fetch", vi.fn().mockResolvedValue(new Response("No such session.", { status: 404 })));
    await expect(sessions.restart(endpoint, "t1", "plan", { cols: 80, rows: 24 })).rejects.toBeInstanceOf(ApiError);
  });
});

describe("the shell panel", () => {
  it("opens it with a folder and size, and returns the tab id", async () => {
    const open = respond({ tab_id: "p1" });
    vi.stubGlobal("fetch", open);
    await expect(sessions.shellPanel(endpoint, "C:\\work", { cols: 70, rows: 20 })).resolves.toBe("p1");
    expect(call(open)).toMatchObject({
      url: "http://127.0.0.1:9/v1/shell-panel",
      method: "POST",
      body: { cwd: "C:\\work", cols: 70, rows: 20 },
    });
  });

  it("adopts it and returns the tab id", async () => {
    const adopt = respond({ tab_id: "p1" });
    vi.stubGlobal("fetch", adopt);
    await expect(sessions.adoptShellPanel(endpoint)).resolves.toBe("p1");
    expect(call(adopt)).toMatchObject({ url: "http://127.0.0.1:9/v1/shell-panel/adopt", method: "POST" });
  });
});

describe("profiles and projects", () => {
  it("gets and puts profiles", async () => {
    const doc = { profiles: [], default_id: "x" };
    const get = respond(doc);
    vi.stubGlobal("fetch", get);
    await expect(profiles.get(endpoint)).resolves.toEqual(doc);
    expect(call(get)).toMatchObject({ url: "http://127.0.0.1:9/v1/profiles", method: "GET" });

    const put = respond(doc);
    vi.stubGlobal("fetch", put);
    await profiles.put(endpoint, doc);
    expect(call(put)).toMatchObject({ url: "http://127.0.0.1:9/v1/profiles", method: "PUT", body: doc });
  });

  it("lists projects", async () => {
    const list = respond([]);
    vi.stubGlobal("fetch", list);
    await projects.list(endpoint);
    expect(call(list)).toMatchObject({ url: "http://127.0.0.1:9/v1/projects", method: "GET" });
  });
});
