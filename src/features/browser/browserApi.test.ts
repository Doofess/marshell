import { beforeEach, describe, expect, it, vi } from "vitest";

const calls: string[] = [];
const pending = new Map<string, () => void>();
const invoke = vi.fn((command: string) => {
  calls.push(command);
  if (command === "browser_screenshot") return new Promise(() => {});
  if (command === "browser_show") return new Promise<void>((r) => pending.set(command, r));
  return Promise.resolve();
});
vi.mock("@tauri-apps/api/core", () => ({ invoke: (command: string) => invoke(command) }));
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn() }));
vi.mock("../../lib/window/appWindow", () => ({ inTauri: () => true }));

import { browser, type Report } from "./browserApi";

const flush = () => new Promise<void>((r) => setTimeout(r, 0));

describe("browser.screenshot", () => {
  beforeEach(() => {
    calls.length = 0;
    pending.clear();
  });

  it("a screenshot waits for the calls before it and holds up none after it", async () => {
    void browser.show("s/p");
    void browser.screenshot("s/p", false);
    void browser.place({} as Report);
    await flush();
    expect(calls).toEqual(["browser_show"]);
    pending.get("browser_show")!();
    await flush();
    expect(calls).toEqual(["browser_show", "browser_screenshot", "browser_place"]);
  });
});
