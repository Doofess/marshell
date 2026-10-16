import { afterEach, describe, expect, it, vi } from "vitest";
import type { Endpoint } from "../../generated/Endpoint";
import { ApiError } from "./client";
import { tasksApi } from "./tasks";

const endpoint: Endpoint = { port: 9, token: "t", platform: "windows" };
const respond = (body: unknown, status = 200) =>
  vi.fn().mockResolvedValue(new Response(body === null ? null : JSON.stringify(body), { status }));
const call = (fetchMock: ReturnType<typeof vi.fn>) => {
  const [url, init] = fetchMock.mock.calls[0]!;
  return { url: url as string, method: init.method as string, body: init.body ? JSON.parse(init.body) : undefined };
};

afterEach(() => vi.unstubAllGlobals());

describe("tasks api", () => {
  it("lists, plans and creates", async () => {
    const list = respond([]);
    vi.stubGlobal("fetch", list);
    await tasksApi.list(endpoint);
    expect(call(list)).toMatchObject({ url: "http://127.0.0.1:9/v1/tasks", method: "GET" });

    const plan = respond({ commands: ["git worktree add"] });
    vi.stubGlobal("fetch", plan);
    await tasksApi.plan(endpoint, { cwd: "C:/a", name: "x" });
    expect(call(plan)).toMatchObject({ url: "http://127.0.0.1:9/v1/tasks/plan", method: "POST", body: { cwd: "C:/a", name: "x" } });

    const create = respond({ id: "t1", path: "C:/w" });
    vi.stubGlobal("fetch", create);
    await expect(tasksApi.create(endpoint, { cwd: "C:/a", name: "x" })).resolves.toEqual({ id: "t1", path: "C:/w" });
    expect(call(create)).toMatchObject({ url: "http://127.0.0.1:9/v1/tasks", method: "POST", body: { cwd: "C:/a", name: "x" } });
  });

  it("reads changes and plans merge and remove, encoding the id", async () => {
    const changes = respond({ files: [] });
    vi.stubGlobal("fetch", changes);
    await tasksApi.changes(endpoint, "a/b");
    expect(call(changes)).toMatchObject({ url: "http://127.0.0.1:9/v1/tasks/a%2Fb/changes", method: "GET" });

    const req = { message: "m", remove: true, commands: ["git merge"] };
    const mergePlan = respond({ commands: [], blocker: null });
    vi.stubGlobal("fetch", mergePlan);
    await tasksApi.mergePlan(endpoint, "t1", req);
    expect(call(mergePlan)).toMatchObject({ url: "http://127.0.0.1:9/v1/tasks/t1/merge/plan", method: "POST", body: req });

    const merge = respond({ kind: "merged", removed: true });
    vi.stubGlobal("fetch", merge);
    await expect(tasksApi.merge(endpoint, "t1", req)).resolves.toEqual({ kind: "merged", removed: true });
    expect(call(merge)).toMatchObject({ url: "http://127.0.0.1:9/v1/tasks/t1/merge", method: "POST", body: req });

    const removePlan = respond({ commands: [], warnings: [], sessions: 0 });
    vi.stubGlobal("fetch", removePlan);
    await tasksApi.removePlan(endpoint, "t1");
    expect(call(removePlan)).toMatchObject({ url: "http://127.0.0.1:9/v1/tasks/t1/remove/plan", method: "POST" });

    const remove = respond(null, 204);
    vi.stubGlobal("fetch", remove);
    await expect(tasksApi.remove(endpoint, "t1", { commands: ["c"], warnings: ["w"] })).resolves.toBeUndefined();
    expect(call(remove)).toMatchObject({ url: "http://127.0.0.1:9/v1/tasks/t1/remove", method: "POST", body: { commands: ["c"], warnings: ["w"] } });
  });

  it("turns a 409 into an ApiError that carries the core's reason as its message", async () => {
    vi.stubGlobal("fetch", respond({ reason: "This folder is not a git repository." }, 409));
    const err = await tasksApi.plan(endpoint, { cwd: "C:/a", name: "" }).catch((e: unknown) => e);
    expect(err).toBeInstanceOf(ApiError);
    expect(err).toMatchObject({ status: 409, message: "This folder is not a git repository." });
  });

  it("keeps the plain text when a 409 is not our JSON, and leaves other failures alone", async () => {
    vi.stubGlobal("fetch", vi.fn().mockResolvedValue(new Response("nope", { status: 409 })));
    await expect(tasksApi.remove(endpoint, "t1", { commands: [], warnings: [] })).rejects.toMatchObject({ status: 409, message: "nope" });
    vi.stubGlobal("fetch", vi.fn().mockResolvedValue(new Response("", { status: 404 })));
    await expect(tasksApi.changes(endpoint, "gone")).rejects.toMatchObject({ status: 404 });
  });
});
