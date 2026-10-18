import { afterEach, describe, expect, it, vi } from "vitest";
import { pickFolder } from "./folderPicker";

const invoke = vi.hoisted(() => vi.fn());
vi.mock("@tauri-apps/api/core", () => ({ invoke }));

afterEach(() => invoke.mockReset());

describe("pickFolder", () => {
  it("passes the start folder and returns the picked one", async () => {
    invoke.mockResolvedValue("C:\\work");
    expect(await pickFolder("C:\\start")).toBe("C:\\work");
    expect(invoke).toHaveBeenCalledWith("pick_folder", { start: "C:\\start" });
  });

  it("passes null through and maps a cancel to null", async () => {
    invoke.mockResolvedValue(null);
    expect(await pickFolder(null)).toBeNull();
    expect(invoke).toHaveBeenCalledWith("pick_folder", { start: null });
  });
});
