import type { Meta, StoryObj } from "@storybook/react-vite";
import type { ReactNode } from "react";
import { ThemePair } from "../../design/ThemePair";
import { FieldRow } from "./FieldRow";
import { ScopeBadge } from "./ScopeBadge";
import { SettingsPage } from "./SettingsPage";
import { SCOPES } from "./scope";
import type { SettingField } from "./fields";
import { SETTINGS_TEXT, SETTINGS_TEXT_BROKEN, SETTINGS_TEXT_LONG } from "./fixtures";
import type { SettingsState } from "./settingsUi";

const meta: Meta = { title: "Settings", parameters: { layout: "fullscreen" } };
export default meta;

const frame = (children: ReactNode, width = 960, height = 940) => <div style={{ inlineSize: width, maxInlineSize: "100%", blockSize: height }}>{children}</div>;
const page = (label: string, props: { category?: string; mode?: "form" | "raw"; state?: SettingsState; edits?: Record<string, unknown>; opened?: string } = {}, width?: number, height?: number) => (
  <ThemePair label={label} stack>
    {frame(<SettingsPage level={2} category={props.category ?? "claude"} mode={props.mode ?? "form"} state={props.state ?? {}} edits={props.edits} opened={props.opened} />, width, height)}
  </ThemePair>
);

export const FormMode: StoryObj = { render: () => page("Form mode: every value shows where it comes from, what it does and when a change takes effect") };
export const RawMode: StoryObj = { render: () => page("Raw mode: the file as text, in the mono font", { mode: "raw", opened: SETTINGS_TEXT }) };
export const RawModeWithError: StoryObj = { render: () => page("Raw mode: a mistake is named by line and column, and linked to the text area", { mode: "raw", opened: SETTINGS_TEXT_BROKEN }) };
export const ManagedLocked: StoryObj = { render: () => page("A value set by your organisation is locked, with the reason, and cannot be edited") };
export const PendingChange: StoryObj = { render: () => page("One change waiting: Review changes comes before any diff", { state: { changed: true }, edits: { cleanupPeriodDays: 365 }, opened: SETTINGS_TEXT }) };
export const ReviewingChange: StoryObj = { render: () => page("Under review: the exact diff, where the backup goes and how to undo, then Apply and Discard", { state: { changed: true, reviewing: true }, edits: { cleanupPeriodDays: 365 }, opened: SETTINGS_TEXT }, 960, 1100) };
export const ReviewingLongChange: StoryObj = { render: () => page("A change of 200 lines folds after 40 lines and stays inside the page", { state: { changed: true, reviewing: true }, edits: { cleanupPeriodDays: 365 }, opened: SETTINGS_TEXT_LONG }, 960, 760) };
export const ReadOnlyFile: StoryObj = { render: () => page("A read-only file says so and nothing can be applied", { state: { readOnly: true } }) };
export const UnparsableFile: StoryObj = { render: () => page("A file that does not parse fails closed: it is shown, never offered for editing", { mode: "raw", state: { unparsable: true }, opened: SETTINGS_TEXT_BROKEN }) };
export const Conflict: StoryObj = { render: () => page("The file changed on disk after it was opened: review the difference or reload; nothing is overwritten and there is no Keep mine", { state: { conflict: true, changed: true }, edits: { cleanupPeriodDays: 365 } }) };
export const NoAgents: StoryObj = { render: () => page("A section with nothing to show says so and offers a way forward", { category: "codex" }) };
export const NarrowWindow: StoryObj = { render: () => page("At 640 px wide the sections sit above the values and wrap", {}, 640, 680) };

export const ScopeBadges: StoryObj = {
  render: () => (
    <ThemePair label="The five scopes: each is a word and a shape, and the tooltip says what it means" stack>
      <div style={{ display: "flex", flexWrap: "wrap", gap: 12, padding: 16 }}>
        {SCOPES.map((s) => (
          <ScopeBadge key={s} scope={s} />
        ))}
      </div>
    </ThemePair>
  ),
};

const HEBREW: SettingField[] = [
  { category: "general", key: "a", label: "מחק תמלולים אחרי (ימים)", help: "Claude מוחק תמלולים משלו אחרי זמן זה.", kind: "number", value: 30, scope: "user", applies: "new-sessions" },
  { category: "general", key: "b", label: "عنوان بالعربية", help: "وصف قصير يظهر تحت العنوان.", kind: "text", value: "", scope: "project", applies: "live" },
];
export const RtlLabels: StoryObj = {
  render: () => (
    <ThemePair label="Hebrew and Arabic labels keep their direction, and the badge stays on the end edge" stack>
      <ul style={{ listStyle: "none", margin: 0, padding: 16, display: "grid", gap: 8, maxInlineSize: 640 }}>
        {HEBREW.map((f) => (
          <FieldRow key={f.key} field={f} />
        ))}
      </ul>
    </ThemePair>
  ),
};
