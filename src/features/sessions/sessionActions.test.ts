import { describe, expect, it, vi } from "vitest";
import type { Endpoint } from "../../generated/Endpoint";
import type { SessionInfo } from "../../generated/SessionInfo";
import { SessionActions } from "./sessionActions";

const endpoint = { port: 1, token: "t", platform: "windows" } as Endpoint;
const info = (tab_id: string): SessionInfo => ({ tab_id, number: 1, name: tab_id, agent: null, profile_id: "pwsh", cwd: "C:/x", project: "C:/x", state: { kind: "running" } }) as SessionInfo;

function setup(selectedId: string | undefined) {
  const close = vi.fn().mockResolvedValue(undefined);
  const actions = new SessionActions({
    api: { close, reopen: vi.fn(), rename: vi.fn(), reorder: vi.fn(), resume: vi.fn() },
    context: () => ({ endpoint, infos: [info("a"), info("b")], selectedId }),
    select: vi.fn(),
  });
  return { actions, close };
}

describe("an explicit tab id", () => {
  it("requestClose closes that session, not the selected one", async () => {
    const { actions, close } = setup("a");
    await actions.requestClose("b");
    expect(close).toHaveBeenCalledWith(endpoint, "b");
  });
  it("startRename renames that session", () => {
    const { actions } = setup("a");
    actions.startRename("b");
    expect(actions.getState().renaming).toBe("b");
  });
  it("a call with no id still acts on the selected session", async () => {
    const { actions, close } = setup("a");
    actions.startRename();
    expect(actions.getState().renaming).toBe("a");
    await actions.requestClose();
    expect(close).toHaveBeenCalledWith(endpoint, "a");
  });
  it("an id that is not a session does nothing", async () => {
    const { actions, close } = setup("a");
    actions.startRename("gone");
    await actions.requestClose("gone");
    expect(actions.getState().renaming).toBeNull();
    expect(close).not.toHaveBeenCalled();
  });
});
