import { afterEach, describe, expect, it, vi } from "vitest";
import { pickFile } from "./filePicker";

const invoke = vi.hoisted(() => vi.fn());
vi.mock("@tauri-apps/api/core", () => ({ invoke }));

afterEach(() => invoke.mockReset());

describe("pickFile", () => {
  it("passes the start folder and returns the picked file", async () => {
    invoke.mockResolvedValue("C:\\work\\README.md");
    expect(await pickFile("C:\\work")).toBe("C:\\work\\README.md");
    expect(invoke).toHaveBeenCalledWith("pick_file", { start: "C:\\work" });
  });

  it("passes null through and maps a cancel to null", async () => {
    invoke.mockResolvedValue(null);
    expect(await pickFile(null)).toBeNull();
    expect(invoke).toHaveBeenCalledWith("pick_file", { start: null });
  });
});
