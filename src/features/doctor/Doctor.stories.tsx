import type { Meta, StoryObj } from "@storybook/react-vite";
import type { ReactNode } from "react";
import { ThemePair } from "../../design/ThemePair";
import { unifiedDiff } from "../../lib/diff";
import { DoctorPage, type DoctorPageProps, type DoctorPhase } from "./DoctorPage";
import { DifferencesSheet, FixSheet } from "./FixSheet";
import { DOCTOR_CLEAR, DOCTOR_CLI, DOCTOR_PLAN, DOCTOR_PREVIEW, DOCTOR_PROBLEMS, DOCTOR_REPORT, UNDO_FIXED, filePreview } from "./fixtures";
import { initialSelection } from "./report";
import type { DoctorReport } from "./types";

const meta: Meta = { title: "Doctor", parameters: { layout: "fullscreen" } };
export default meta;

const noop = () => {};
const frame = (children: ReactNode, height = 800) => <div style={{ blockSize: height || undefined, maxInlineSize: 900 }}>{children}</div>;
const page = (report: DoctorReport | null, phase: DoctorPhase, extra: Partial<DoctorPageProps> = {}) =>
  frame(
    <DoctorPage
      report={report}
      phase={phase}
      selected={report ? initialSelection(report) : new Set()}
      nowMs={(report?.checked_at_ms ?? 0) + 12_000}
      onToggle={noop}
      onFix={noop}
      onRunAgain={noop}
      cliRunning={false}
      onRunCliDoctors={noop}
      onUndo={noop}
      onShowDifferences={noop}
      onOpenAgents={noop}
      onRestart={noop}
      {...extra}
    />,
  );
const sheet = (plan = DOCTOR_PLAN, over: Partial<Parameters<typeof FixSheet>[0]> = {}) => <FixSheet plan={plan} busy={false} conflict={false} onConfirm={noop} onCancel={noop} onReload={noop} {...over} />;

export const Ready: StoryObj = { render: () => <ThemePair label="Doctor: Broken and Info groups; a checkbox on each fixable row, Broken ones checked; retention is Info and unchecked; managed settings are Info with no control" stack>{page(DOCTOR_REPORT, "ready")}</ThemePair> };
export const NothingChosen: StoryObj = { render: () => <ThemePair label="Doctor: with no row checked, the fix button is disabled" stack>{page(DOCTOR_REPORT, "ready", { selected: new Set() })}</ThemePair> };
export const ReadyProblems: StoryObj = { render: () => <ThemePair label="Doctor: problems Marshell will not fix say why, in words, and Antigravity's row offers Open Agents" stack>{page(DOCTOR_PROBLEMS, "ready")}</ThemePair> };
export const CliDoctorsRan: StoryObj = { render: () => <ThemePair label="Doctor: after Run CLI doctors, each CLI's problems are report-only rows that say Marshell only reports them, and the status line says when they ran" stack>{page(DOCTOR_CLI, "ready")}</ThemePair> };
export const CliDoctorsRunning: StoryObj = { render: () => <ThemePair label="Doctor: while the CLI doctors run, the button reads Running CLI doctors… and stays focusable but does nothing" stack>{page(DOCTOR_CLI, "ready", { cliRunning: true })}</ThemePair> };
export const AllClear: StoryObj = { render: () => <ThemePair label="Doctor: all clear, with only Run again" stack>{page(DOCTOR_CLEAR, "ready")}</ThemePair> };
export const Running: StoryObj = { render: () => <ThemePair label="Doctor: a check that takes over 150 ms says what it is checking; a quicker one shows nothing" stack>{page(null, "running", { elapsedMs: 400 })}</ThemePair> };
export const Failed: StoryObj = { render: () => <ThemePair label="Doctor: when it cannot run it says why and offers Run again" stack>{page(null, "failed", { failure: "The core is not running." })}</ThemePair> };

export const FixSheetOpen: StoryObj = { render: () => <ThemePair label="Fix sheet: the exact change to each file, the backup, how to undo and how many sessions need a restart; focus starts on the title" stack>{frame(sheet(), 0)}</ThemePair> };
export const FixSheetConflict: StoryObj = { render: () => <ThemePair label="Fix sheet: a file changed after the preview, so nothing was written; Reload and Cancel only" stack>{frame(sheet(DOCTOR_PLAN, { conflict: true }), 0)}</ThemePair> };
export const FixSheetBusy: StoryObj = { render: () => <ThemePair label="Fix sheet: while the fix is applied the button says Fixing…" stack>{frame(sheet(DOCTOR_PLAN, { busy: true }), 0)}</ThemePair> };

const longBefore = Array.from({ length: 200 }, (_, i) => `    "rule-${i}": "allow",`).join("\n");
const longAfter = Array.from({ length: 200 }, (_, i) => `    "rule-${i}": "${i % 2 ? "allow" : "ask"}",`).join("\n");
const LONG = { ...DOCTOR_PLAN, preview: { ...DOCTOR_PREVIEW, files: [filePreview("C:\\Users\\Home Office\\.claude\\settings.json", unifiedDiff("settings.json", `{\n${longBefore}\n}\n`, `{\n${longAfter}\n}\n`)), filePreview("C:\\Users\\Home Office\\.marshell\\old.json", "", true)] } };
export const FixSheetLong: StoryObj = { render: () => <ThemePair label="Fix sheet: a 200-line change folds after 40 lines, with Show all" stack>{frame(sheet(LONG), 0)}</ThemePair> };
export const FixSheetSmallWindow: StoryObj = { render: () => <ThemePair label="Fix sheet in a 720 by 480 window: the changes scroll in the body; Cancel and the fix stay in view" stack><div style={{ inlineSize: 720, blockSize: 480 }}>{sheet(LONG)}</div></ThemePair> };
export const Differences: StoryObj = { render: () => <ThemePair label="Show differences: what the files say now, with Close" stack>{frame(<DifferencesSheet preview={DOCTOR_PREVIEW} onClose={noop} />, 0)}</ThemePair> };

export const Fixed: StoryObj = { render: () => <ThemePair label="After the fix: each fixed row says Fixed, and the receipt shows for 10 s with Undo and Restart them" stack>{page(DOCTOR_REPORT, "fixed", { undo: UNDO_FIXED, nowMs: UNDO_FIXED.fixed_at_ms + 2_000 })}</ThemePair> };
export const FixedLater: StoryObj = { render: () => <ThemePair label="Later: the toast is gone and Undo last fix stays in the Doctor header" stack>{page(DOCTOR_REPORT, "fixed", { undo: UNDO_FIXED, nowMs: UNDO_FIXED.fixed_at_ms + 60_000 })}</ThemePair> };
export const UndoStale: StoryObj = { render: () => <ThemePair label="A file changed since the fix: undo is withdrawn and the page says why and offers the differences" stack>{page(DOCTOR_REPORT, "fixed", { undo: { ...UNDO_FIXED, files_match: false }, nowMs: UNDO_FIXED.fixed_at_ms + 60_000 })}</ThemePair> };

const RTL: DoctorReport = {
  ...DOCTOR_REPORT,
  items: DOCTOR_REPORT.items.map((i) => (i.id === "retention.claude" ? { ...i, title: "Claude מוחק תמלולים אחרי 30 יום", path: "C:\\Users\\רותם\\.claude\\settings.json" } : i)),
};
export const RtlFilePath: StoryObj = { render: () => <ThemePair label="A Hebrew title and a path with Hebrew in it stay readable and wrap" stack>{page(RTL, "ready")}</ThemePair> };
