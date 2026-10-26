import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it, vi } from "vitest";
import type { Endpoint } from "../../generated/Endpoint";
import type { IntegrationInfo } from "../../generated/IntegrationInfo";
import type { Profile } from "../../generated/Profile";
import type { ProfilesDoc } from "../../generated/ProfilesDoc";
import { ApiError } from "../../lib/api/client";
import { splitCommand, joinCommand } from "./profilesDoc";
import { canClose, ProfilesView, type ProfilesHandlers } from "./ProfilesSection";
import { ProfilesStore } from "./ProfilesStore";

const ep: Endpoint = { port: 1, token: "t", platform: "windows" };
const profile = (id: string, patch: Partial<Profile> = {}): Profile => ({ id, name: `Name ${id}`, command: [`${id}.exe`], family: "cmd", cwd: null, env: [], shell_integration: true, detected: false, ...patch });
const FOUND: Profile[] = [profile("pwsh", { detected: true }), profile("cmd", { detected: true })];
const DOC: ProfilesDoc = { profiles: FOUND, default_id: "pwsh" };

const SCRIPT: IntegrationInfo = { applies: true, reason: null, script_path: "C:\\Users\\me\\.marshell\\shell\\marshell.ps1", script: "function prompt { }" };

function setup(over: { put?: (ep: Endpoint, doc: ProfilesDoc) => Promise<ProfilesDoc>; doc?: ProfilesDoc; integration?: (ep: Endpoint, p: Profile) => Promise<IntegrationInfo> } = {}) {
  const put = vi.fn(over.put ?? (async (_: Endpoint, doc: ProfilesDoc) => doc));
  const integration = vi.fn(over.integration ?? (async () => SCRIPT));
  const api = { get: vi.fn(async () => over.doc ?? DOC), detected: vi.fn(async () => FOUND), put, integration };
  return { api, put, integration, store: new ProfilesStore(api, ep) };
}

const noop = () => {};
const handlers: ProfilesHandlers = { onSelect: noop, onPatch: noop, onAdd: noop, onRemove: noop, onMakeDefault: noop, onReset: noop, onSave: noop, onDiscard: noop, onToggleScript: noop };
const html = (store: ProfilesStore) => renderToStaticMarkup(<ProfilesView state={store.getState()} handlers={handlers} />);

describe("ProfilesSection", () => {
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

  it("tells the window after a save, so new sessions use the saved profiles, and not after a refusal", async () => {
    const onSaved = vi.fn();
    const ok = setup();
    const store = new ProfilesStore(ok.api, ep, onSaved);
    await store.load();
    store.patch({ name: "x" });
    await store.save();
    expect(onSaved).toHaveBeenCalledTimes(1);
    const refused = setup({ put: async () => Promise.reject(new ApiError(400, "no")) });
    const failing = new ProfilesStore(refused.api, ep, onSaved);
    await failing.load();
    failing.patch({ name: "y" });
    await failing.save();
    expect(onSaved).toHaveBeenCalledTimes(1);
  });

  it("command field round-trips quoted args", () => {
    const text = '"C:\\Program Files\\Git\\bin\\bash.exe" -l';
    const argv = ["C:\\Program Files\\Git\\bin\\bash.exe", "-l"];
    expect(splitCommand(text)).toEqual(argv);
    expect(joinCommand(argv)).toBe(text);
  });
});

describe("ProfilesSection: the rest of the page", () => {
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
    const api = { get: vi.fn(async () => Promise.reject(new Error("offline"))), detected: vi.fn(async () => []), put: vi.fn(), integration: vi.fn() };
    const store = new ProfilesStore(api, ep);
    await store.load();
    expect(html(store)).toContain("Couldn&#x27;t load the profiles: offline");
  });
});

describe("ProfilesSection: a command the one-line field cannot write", () => {
  // The kind of argv a hand-edited profiles.json holds.
  const QUOTED = ["sh", "-c", 'echo "hi"'];
  it("renders (no re-render loop), read-only, with a note, and keeps the argv", async () => {
    const { store } = setup({ doc: { profiles: [profile("pwsh", { detected: true }), profile("cmd", { detected: true, command: QUOTED })], default_id: "pwsh" } });
    await store.load();
    store.select("cmd");
    const h = html(store);
    expect(h).toMatch(/<input[^>]*readOnly=""[^>]*value="sh -c echo/);
    expect(h).toContain("quote mark");
    expect(store.getState().draft!.profiles[1]!.command).toEqual(QUOTED);
  });
  it("a command that can be written has no note and an editable box", async () => {
    const { store } = setup();
    await store.load();
    const h = html(store);
    expect(h).not.toContain("quote mark");
    expect(h).not.toMatch(/<input[^>]*readOnly=""[^>]*value="pwsh.exe"/);
  });
});

describe("ProfilesSection: leaving with unsaved edits", () => {
  it("draws no Close button, title or dialog shell: the Settings page owns them", async () => {
    const { store } = setup();
    await store.load();
    const h = html(store);
    expect(h).not.toContain(">Close</button>");
    expect(h).not.toContain('role="dialog"');
    expect(h).not.toMatch(/<h1/);
    expect(h).not.toContain("Shell profiles");
  });
  it("keeps the change bar while there are unsaved edits, and canClose tells the owner", async () => {
    const { store } = setup();
    await store.load();
    expect(canClose(store.getState())).toBe(true);
    store.patch({ name: "Edited" });
    expect(html(store)).toContain("change-bar");
    expect(canClose(store.getState())).toBe(false);
    store.discard();
    expect(canClose(store.getState())).toBe(true);
  });
});

describe("ProfilesSection: shell integration", () => {
  it("shows a Shell integration switch that turns it off in the saved doc", async () => {
    const { store, put } = setup();
    await store.load();
    expect(html(store)).toMatch(/<input[^>]*role="switch"[^>]*checked=""[^>]*>[\s\S]*?Shell integration/);
    store.patch({ shell_integration: false });
    expect(html(store)).not.toMatch(/<input[^>]*role="switch"[^>]*checked=""/);
    await store.save();
    expect(put.mock.calls[0]![1].profiles[0]!.shell_integration).toBe(false);
  });

  it("View script asks the core about the edited profile and shows the path and the script, read-only", async () => {
    const { store, integration } = setup();
    await store.load();
    expect(html(store)).toMatch(/<button[^>]*aria-expanded="false"[^>]*>View script<\/button>/);
    store.patch({ name: "Edited pwsh" });
    await store.toggleScript();
    expect(integration).toHaveBeenCalledTimes(1);
    expect(integration.mock.calls[0]![1]).toMatchObject({ id: "pwsh", name: "Edited pwsh" });
    const h = html(store);
    expect(h).toMatch(/<button[^>]*aria-expanded="true"[^>]*>View script<\/button>/);
    expect(h).toContain("C:\\Users\\me\\.marshell\\shell\\marshell.ps1");
    expect(h).toMatch(/<pre[^>]*>function prompt \{ \}<\/pre>/);
    // Closing it hides the script; another profile starts closed.
    await store.toggleScript();
    expect(html(store)).not.toContain("<pre");
    await store.toggleScript();
    store.select("cmd");
    expect(html(store)).not.toContain("<pre");
  });

  it("says why when integration doesn't apply", async () => {
    const { store } = setup({ integration: async () => ({ applies: false, reason: "This shell has no integration script yet.", script_path: null, script: null }) });
    await store.load();
    await store.toggleScript();
    const h = html(store);
    expect(h).toContain("This shell has no integration script yet.");
    expect(h).not.toContain("<pre");
  });

  it("a failed fetch says so and does not throw", async () => {
    const { store } = setup({ integration: async () => Promise.reject(new Error("offline")) });
    await store.load();
    await expect(store.toggleScript()).resolves.toBeUndefined();
    expect(html(store)).toContain("Couldn&#x27;t load the script.");
  });
});

describe("ProfilesSection: environment variables named twice", () => {
  it("says which one, and not otherwise", async () => {
    const { store } = setup();
    await store.load();
    store.patch({ env: [["Path", "1"], [" PATH", "2"]] });
    expect(html(store)).toContain("Path is listed twice");
    store.patch({ env: [["Path", "1"], ["Other", "2"]] });
    expect(html(store)).not.toContain("listed twice");
  });
});

describe("ProfilesSection: terminal theme", () => {
  it("profile terminal theme select offers same-as-app and imported themes", async () => {
    const { store } = setup({ doc: { profiles: [profile("pwsh", { detected: true, terminal_theme: "wt:nord" }), profile("cmd", { detected: true })], default_id: "pwsh" } });
    await store.load();
    const h = renderToStaticMarkup(<ProfilesView state={store.getState()} handlers={handlers} themes={[{ id: "wt:nord", name: "Nord" }]} />);
    const select = h.match(/<select[^>]*>.*?<\/select>/s)?.[0] ?? "";
    expect(select).toContain('<option value="">Same as the app setting</option>');
    expect(select).toContain('<option value="dark">Dark</option>');
    expect(select).toContain('<option value="light">Light</option>');
    expect(select).toMatch(/<option value="wt:nord" selected="">Nord<\/option>/);
    expect(h).toContain("Terminal theme");
  });
  it("a terminal theme change counts as an unsaved change", async () => {
    const { store } = setup();
    await store.load();
    store.patch({ terminal_theme: "light" });
    expect(store.getState().draft!.profiles[0]!.terminal_theme).toBe("light");
    expect(canClose(store.getState())).toBe(false);
  });
});
