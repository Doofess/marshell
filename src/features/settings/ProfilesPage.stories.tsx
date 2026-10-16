import type { Meta, StoryObj } from "@storybook/react-vite";
import type { Profile } from "../../generated/Profile";
import type { ProfilesDoc } from "../../generated/ProfilesDoc";
import { ThemePair } from "../../design/ThemePair";
import { ProfilesPageView, type ProfilesHandlers } from "./ProfilesPage";
import { updateProfile } from "./profilesDoc";
import type { ProfilesState } from "./ProfilesStore";

const meta: Meta = { title: "Settings/Shell profiles", parameters: { layout: "fullscreen" } };
export default meta;

const profile = (id: string, name: string, command: string[], patch: Partial<Profile> = {}): Profile => ({ id, name, command, family: "power_shell", cwd: null, env: [], shell_integration: true, detected: true, ...patch });
const DETECTED: Profile[] = [
  profile("pwsh", "PowerShell 7", ["C:\\Program Files\\PowerShell\\7\\pwsh.exe", "-NoLogo"]),
  profile("powershell", "Windows PowerShell", ["C:\\Windows\\System32\\WindowsPowerShell\\v1.0\\powershell.exe"]),
  profile("cmd", "Command Prompt", ["C:\\Windows\\System32\\cmd.exe"], { family: "cmd" }),
  profile("git-bash", "Git Bash", ["C:\\Program Files\\Git\\bin\\bash.exe", "-l"], { family: "posix" }),
];
const DOC: ProfilesDoc = { profiles: DETECTED, default_id: "pwsh" };
const WITH_CUSTOM: ProfilesDoc = {
  profiles: [...DETECTED, profile("custom-1", "PowerShell in my-app", ["C:\\Program Files\\PowerShell\\7\\pwsh.exe", "-NoLogo"], { cwd: "C:\\dev\\my-app", env: [["NODE_ENV", "development"], ["DEBUG", "app:*"]], detected: false })],
  default_id: "pwsh",
};

const state = (over: Partial<ProfilesState> = {}): ProfilesState => ({
  phase: "ready",
  loadError: null,
  saved: DOC,
  draft: DOC,
  detected: DETECTED,
  selectedId: "pwsh",
  saving: false,
  error: null,
  justSaved: false,
  ...over,
});
const withDoc = (doc: ProfilesDoc, over: Partial<ProfilesState> = {}) => state({ saved: doc, draft: doc, ...over });

const noop = () => {};
const handlers: ProfilesHandlers = { onSelect: noop, onPatch: noop, onAdd: noop, onRemove: noop, onMakeDefault: noop, onReset: noop, onSave: noop, onDiscard: noop, onClose: noop };
const show = (label: string, s: ProfilesState, height = 760) => (
  <ThemePair label={label} stack>
    <div style={{ inlineSize: 960, maxInlineSize: "100%", blockSize: height }}>
      <ProfilesPageView state={s} handlers={handlers} />
    </div>
  </ThemePair>
);

export const Detected: StoryObj = { render: () => show("Detected profiles: each says it was found here, and Delete is off", state()) };
export const CustomProfile: StoryObj = { render: () => show("A profile you added: a folder, variables, and Delete", withDoc(WITH_CUSTOM, { selectedId: "custom-1" })) };
export const GitBashQuotedPath: StoryObj = { render: () => show("A path with spaces is quoted in the command field", state({ selectedId: "git-bash" })) };

const edited = updateProfile(DOC, "cmd", { command: ["C:\\Windows\\System32\\cmd.exe", "/k", "chcp 65001"] });
export const EditedDetected: StoryObj = { render: () => show("An edited detected profile: marked Edited, Reset is on, and the bar waits for Save", state({ draft: edited, selectedId: "cmd" })) };
export const RefusedSave: StoryObj = {
  render: () => show("The core refused the save: its words show in the bar and the edits stay", state({ draft: updateProfile(DOC, "cmd", { command: [] }), selectedId: "cmd", error: 'Profile "Command Prompt" needs a command.' })),
};
export const FileCannotBeWritten: StoryObj = {
  render: () => show("The profiles file cannot be written: the core's message shows as it is", state({ draft: edited, selectedId: "cmd", error: "profiles.json was written by a newer Marshell (version 9)" })),
};
export const Saved: StoryObj = { render: () => show("Saved: the bar is gone and the page says so", state({ justSaved: true })) };
export const NotLoaded: StoryObj = { render: () => show("The profiles didn't load", state({ phase: "failed", loadError: "Failed to fetch", saved: null, draft: null, selectedId: "" }), 420) };
export const NarrowWindow: StoryObj = {
  render: () => (
    <ThemePair label="At 640 px wide the list sits above the form" stack>
      <div style={{ inlineSize: 640, maxInlineSize: "100%", blockSize: 820 }}>
        <ProfilesPageView state={withDoc(WITH_CUSTOM, { selectedId: "custom-1" })} handlers={handlers} />
      </div>
    </ThemePair>
  ),
};
