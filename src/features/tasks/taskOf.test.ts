import { describe, expect, it } from "vitest";
import type { SessionInfo } from "../../generated/SessionInfo";
import type { Task } from "../../generated/Task";
import { samePath, taskOf } from "./taskOf";

const task = (id: string, path: string): Task => ({ id, name: id, branch: `marshell/${id}`, base_branch: "main", repo_root: "C:\\dev\\app", path, created_ms: 1, folder_missing: false });
const session = (cwd: string) => ({ tab_id: "t1", number: 1, name: "one", agent: null, profile_id: "p", cwd, project: "C:\\dev\\app" }) as unknown as SessionInfo;

const TASKS = [task("a", "C:\\Users\\Home Office\\.marshell\\worktrees\\app\\x"), task("b", "C:\\Users\\Home Office\\.marshell\\worktrees\\app\\x-2")];

describe("taskOf", () => {
  it("matches a trailing separator on the session's folder", () => {
    expect(taskOf(session("C:\\Users\\Home Office\\.marshell\\worktrees\\app\\x\\"), TASKS)?.id).toBe("a");
  });
  it("ignores case on a Windows path", () => {
    expect(taskOf(session("c:\\users\\home office\\.MARSHELL\\worktrees\\app\\X"), TASKS)?.id).toBe("a");
  });
  it("treats / and \\ as the same separator", () => {
    expect(taskOf(session("C:/Users/Home Office/.marshell/worktrees/app/x-2/"), TASKS)?.id).toBe("b");
  });
  it("does not match a folder inside the task, or a sibling with the same prefix", () => {
    expect(taskOf(session("C:\\Users\\Home Office\\.marshell\\worktrees\\app\\x\\src"), TASKS)).toBeUndefined();
    expect(taskOf(session("C:\\Users\\Home Office\\.marshell\\worktrees\\app"), TASKS)).toBeUndefined();
  });
  it("finds nothing for no tasks", () => {
    expect(taskOf(session("C:\\dev\\app"), [])).toBeUndefined();
  });
});

describe("samePath", () => {
  it("keeps a root as a root", () => {
    expect(samePath("C:\\", "C:/")).toBe(true);
    expect(samePath("C:\\", "C:\\dev")).toBe(false);
    expect(samePath("/", "/")).toBe(true);
  });
  it("is case-sensitive for a path that is not a Windows path", () => {
    expect(samePath("/home/a/App", "/home/a/app")).toBe(false);
    expect(samePath("/home/a/app/", "/home/a/app")).toBe(true);
  });
});
