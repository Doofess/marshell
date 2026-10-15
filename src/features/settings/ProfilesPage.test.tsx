import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it, vi } from "vitest";
import type { Endpoint } from "../../generated/Endpoint";
import type { Profile } from "../../generated/Profile";
import type { ProfilesDoc } from "../../generated/ProfilesDoc";
import { ApiError } from "../../lib/api/client";
import { splitCommand, joinCommand } from "./profilesDoc";
import { ProfilesPageView, type ProfilesHandlers } from "./ProfilesPage";
import { ProfilesStore } from "./ProfilesStore";

const ep: Endpoint = { port: 1, token: "t", platform: "windows" };
const profile = (id: string, patch: Partial<Profile> = {}): Profile => ({ id, name: `Name ${id}`, command: [`${id}.exe`], family: "cmd", cwd: null, env: [], detected: false, ...patch });
const FOUND: Profile[] = [profile("pwsh", { detected: true }), profile("cmd", { detected: true })];
const DOC: ProfilesDoc = { profiles: FOUND, default_id: "pwsh" };

function setup(over: { put?: (ep: Endpoint, doc: ProfilesDoc) => Promise<ProfilesDoc>; doc?: ProfilesDoc } = {}) {
  const put = vi.fn(over.put ?? (async (_: Endpoint, doc: ProfilesDoc) => doc));
  const api = { get: vi.fn(async () => over.doc ?? DOC), detected: vi.fn(async () => FOUND), put };
  return { api, put, store: new ProfilesStore(api, ep) };
}

const noop = () => {};
const handlers: ProfilesHandlers = { onSelect: noop, onPatch: noop, onAdd: noop, onRemove: noop, onMakeDefault: noop, onReset: noop, onSave: noop, onDiscard: noop };
const html = (store: ProfilesStore) => renderToStaticMarkup(<ProfilesPageView state={store.getState()} handlers={handlers} />);

describe("ProfilesPage", () => {
  it("edits a detected profile and saves the doc", async () => {
    const { store, put } = setup();
    await store.load();
    store.select("cmd");
    store.patch({ command: splitCommand('"C:\\Windows\\System32\\cmd.exe" /k') });
    store.patch({ name: "My cmd", cwd: "C:\\dev", env: [["FOO", "bar"]] });
    // Nothing is sent until Save.
    expect(put).not.toHaveBeenCalled();
    await store.save();
    expect(put).toHaveBeenCalledTimes(1);
    const sent = put.mock.calls[0]![1];
    expect(sent.default_id).toBe("pwsh");
    expect(sent.profiles[0]).toEqual(FOUND[0]);
    expect(sent.profiles[1]).toMatchObject({ id: "cmd", name: "My cmd", command: ["C:\\Windows\\System32\\cmd.exe", "/k"], cwd: "C:\\dev", env: [["FOO", "bar"]] });
    expect(store.getState().justSaved).toBe(true);
    expect(store.getState().error).toBeNull();
  });

  it("delete is disabled for detected profiles", async () => {
    const { store } = setup();
    await store.load();
    expect(html(store)).toMatch(/<button[^>]*disabled=""[^>]*>Delete<\/button>/);
    // Even after editing it, a detected profile cannot be deleted; Reset is there instead.
    store.patch({ name: "Edited" });
    expect(html(store)).toMatch(/<button[^>]*disabled=""[^>]*>Delete<\/button>/);
    expect(html(store)).toMatch(/<button(?![^>]*disabled)[^>]*>Reset<\/button>/);
    store.remove();
    expect(store.getState().draft!.profiles).toHaveLength(2);
    // A custom copy can be deleted.
    store.add();
    expect(store.getState().selectedId).toBe("custom-1");
    expect(html(store)).toMatch(/<button(?![^>]*disabled)[^>]*>Delete<\/button>/);
    store.remove();
    expect(store.getState().draft!.profiles.map((p) => p.id)).toEqual(["pwsh", "cmd"]);
  });

  it("server validation error shows in the change bar", async () => {
    const message = 'Profile "Name cmd" needs a command.';
    const { store } = setup({ put: async () => Promise.reject(new ApiError(400, message)) });
    await store.load();
    store.select("cmd");
    store.patch({ command: [] });
    await store.save();
    expect(store.getState().error).toBe(message);
    // The edits stay, so the person can fix them.
    expect(store.getState().draft!.profiles[1]!.command).toEqual([]);
    const h = html(store);
    expect(h).toMatch(/class="change-bar"[\s\S]*change-bar__error">Profile &quot;Name cmd&quot; needs a command\.<\/p>/);
    // A 500 from a profiles file the core cannot write is shown the same way, word for word.
    const broken = setup({ put: async () => Promise.reject(new ApiError(500, "profiles.json is damaged")) });
    await broken.store.load();
    broken.store.patch({ name: "x" });
    await broken.store.save();
    expect(html(broken.store)).toContain("profiles.json is damaged");
    // Editing again clears the old refusal.
    broken.store.patch({ name: "y" });
    expect(broken.store.getState().error).toBeNull();
  });

  it("command field round-trips quoted args", () => {
    const text = '"C:\\Program Files\\Git\\bin\\bash.exe" -l';
    const argv = ["C:\\Program Files\\Git\\bin\\bash.exe", "-l"];
    expect(splitCommand(text)).toEqual(argv);
    expect(joinCommand(argv)).toBe(text);
  });
});

describe("ProfilesPage: the rest of the page", () => {
  it("lists the profiles with Detected and Default badges, and the form of the selected one", async () => {
    const { store } = setup();
    await store.load();
    const h = html(store);
    expect(h).toContain("Name pwsh");
    expect(h.match(/>Detected</g)).toHaveLength(2);
    expect(h).toContain(">Default<");
    expect(h).toMatch(/aria-current="true"[^>]*>[\s\S]*?Name pwsh/);
    expect(h).toContain('value="pwsh.exe"');
    expect(h).toMatch(/<button[^>]*disabled=""[^>]*>Make default<\/button>/);
  });

  it("shows no change bar until something changes, and Save and Discard after", async () => {
    const { store } = setup();
    await store.load();
    expect(html(store)).not.toContain("change-bar");
    store.patch({ name: "Other" });
    const h = html(store);
    expect(h).toContain("1 change");
    expect(h).toContain(">Save<");
    expect(h).toContain(">Discard<");
    store.discard();
    expect(html(store)).not.toContain("change-bar");
    expect(store.getState().draft).toBe(store.getState().saved);
  });

  it("Reset goes back to the detected values, and an edited-back profile is clean again", async () => {
    const { store } = setup();
    await store.load();
    store.select("cmd");
    store.patch({ command: ["cmd.exe", "/k"] });
    expect(html(store)).toContain(">Edited<");
    store.reset();
    expect(store.getState().draft!.profiles[1]).toEqual(FOUND[1]);
    expect(html(store)).not.toContain("change-bar");
  });

  it("makes another profile the default and sends it", async () => {
    const { store, put } = setup();
    await store.load();
    store.select("cmd");
    store.makeDefault();
    await store.save();
    expect(put.mock.calls[0]![1].default_id).toBe("cmd");
  });

  it("does not send environment rows without a name", async () => {
    const { store, put } = setup();
    await store.load();
    store.patch({ env: [["A", "1"], ["", ""]] });
    await store.save();
    expect(put.mock.calls[0]![1].profiles[0]!.env).toEqual([["A", "1"]]);
  });

  it("says why it could not load", async () => {
    const api = { get: vi.fn(async () => Promise.reject(new Error("offline"))), detected: vi.fn(async () => []), put: vi.fn() };
    const store = new ProfilesStore(api, ep);
    await store.load();
    expect(html(store)).toContain("Couldn&#x27;t load the profiles: offline");
  });
});
