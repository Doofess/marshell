import { beforeEach, describe, expect, it, vi } from "vitest";

const calls: string[] = [];
const pending = new Map<string, () => void>();
let refuseCookies = false;
const invoke = vi.fn((command: string) => {
  calls.push(command);
  if (command === "browser_screenshot") return new Promise(() => {});
  if (command === "browser_show") return new Promise<void>((r) => pending.set(command, r));
  if (command === "browser_cookies" && refuseCookies) return Promise.reject("no such cookie rule");
  return Promise.resolve();
});
vi.mock("@tauri-apps/api/core", () => ({ invoke: (command: string) => invoke(command) }));
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn() }));
vi.mock("../../lib/window/appWindow", () => ({ inTauri: () => true }));

import type { Report } from "./browserApi";

const flush = () => new Promise<void>((r) => setTimeout(r, 0));

/** The module is loaded fresh for each test: its queue starts closed. */
async function fresh() {
  vi.resetModules();
  return (await import("./browserApi")).browser;
}

beforeEach(() => {
  calls.length = 0;
  pending.clear();
  refuseCookies = false;
});

describe("browser.screenshot", () => {
  it("a screenshot waits for the calls before it and holds up none after it", async () => {
    const browser = await fresh();
    void browser.cookies("always");
    void browser.show("s/p");
    void browser.screenshot("s/p", false);
    void browser.place({} as Report);
    await flush();
    expect(calls).toEqual(["browser_cookies", "browser_show"]);
    pending.get("browser_show")!();
    await flush();
    expect(calls).toEqual(["browser_cookies", "browser_show", "browser_screenshot", "browser_place"]);
  });
});

describe("the cookie rule's gate", () => {
  it("nothing reaches the shell before the first rule is told, and then in the order asked", async () => {
    const browser = await fresh();
    void browser.open("s/a", "https://a.example/");
    void browser.close("s/a");
    await flush();
    expect(calls).toEqual([]);
    await browser.cookies("always");
    await flush();
    expect(calls).toEqual(["browser_cookies", "browser_open", "browser_close"]);
  });
  it("a rule the shell refused still opens the queue", async () => {
    refuseCookies = true;
    const browser = await fresh();
    void browser.open("s/a", "https://a.example/");
    await expect(browser.cookies("never")).rejects.toBe("no such cookie rule");
    await flush();
    expect(calls).toEqual(["browser_cookies", "browser_open"]);
  });
  it("a second rule takes its turn in the queue", async () => {
    const browser = await fresh();
    void browser.cookies("always");
    void browser.open("s/a", "https://a.example/");
    void browser.cookies("never");
    void browser.open("s/b", "https://b.example/");
    await flush();
    expect(calls).toEqual(["browser_cookies", "browser_open", "browser_cookies", "browser_open"]);
  });
});
