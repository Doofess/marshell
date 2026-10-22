import { describe, expect, it, vi } from "vitest";
import { chooseClose } from "./closeChoice";

function windowActions() {
  return {
    isOpen: () => true,
    closeWindow: vi.fn(async () => {}),
    closeOnce: vi.fn(async (_keep: boolean) => {}),
  };
}

describe("chooseClose", () => {
  it.each([true, false])("saves %s before requesting the normal close", async (keep) => {
    const deps = windowActions();
    const save = vi.fn(async () => {
      expect(deps.closeWindow).not.toHaveBeenCalled();
      expect(deps.closeOnce).not.toHaveBeenCalled();
    });
    expect(await chooseClose(keep, { ...deps, save })).toBe("closed");
    expect(save).toHaveBeenCalledExactlyOnceWith(keep);
    expect(deps.closeWindow).toHaveBeenCalledExactlyOnceWith();
    expect(deps.closeOnce).not.toHaveBeenCalled();
  });

  it.each([true, false])("carries out the choice %s once when saving fails", async (keep) => {
    const deps = windowActions();
    const save = async () => {
      throw new Error("Settings are not writable");
    };
    expect(await chooseClose(keep, { ...deps, save })).toBe("closed-once");
    expect(deps.closeWindow).not.toHaveBeenCalled();
    expect(deps.closeOnce).toHaveBeenCalledExactlyOnceWith(keep);
  });

  it.each([true, false])("carries out the choice %s once when saving is unavailable", async (keep) => {
    const deps = windowActions();
    expect(await chooseClose(keep, deps)).toBe("closed-once");
    expect(deps.closeWindow).not.toHaveBeenCalled();
    expect(deps.closeOnce).toHaveBeenCalledExactlyOnceWith(keep);
  });

  it.each([true, false])("does not close after Escape during a save (success: %s)", async (success) => {
    const deps = windowActions();
    let open = true;
    let finish!: () => void;
    const save = () =>
      new Promise<void>((resolve, reject) => {
        finish = () => (success ? resolve() : reject(new Error("Core is down")));
      });
    const result = chooseClose(true, { ...deps, save, isOpen: () => open });
    open = false;
    finish();
    expect(await result).toBe("cancelled");
    expect(deps.closeWindow).not.toHaveBeenCalled();
    expect(deps.closeOnce).not.toHaveBeenCalled();
  });

  it("does not treat a failed window close as a failed save", async () => {
    const deps = windowActions();
    deps.closeWindow.mockRejectedValue(new Error("Window unavailable"));
    await expect(chooseClose(true, { ...deps, save: async () => {} })).rejects.toThrow("Window unavailable");
    expect(deps.closeOnce).not.toHaveBeenCalled();
  });
});
