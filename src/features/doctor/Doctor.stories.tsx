import type { Meta, StoryObj } from "@storybook/react-vite";
import type { ReactNode } from "react";
import { ThemePair } from "../../design/ThemePair";
import { unifiedDiff } from "../../lib/diff";
import { DoctorPage, type DoctorPhase } from "./DoctorPage";
import { FixSheet } from "./FixSheet";
import { DOCTOR_CLEAR, DOCTOR_PROBLEMS, DOCTOR_REPORT, UNDO_FIXED } from "./fixtures";
import type { DoctorReport } from "./types";

const meta: Meta = { title: "Doctor", parameters: { layout: "fullscreen" } };
export default meta;

const frame = (children: ReactNode, height = 800) => <div style={{ blockSize: height || undefined, maxInlineSize: 900 }}>{children}</div>;
const page = (report: DoctorReport, phase: DoctorPhase, extra: Partial<Parameters<typeof DoctorPage>[0]> = {}) => frame(<DoctorPage report={report} phase={phase} nowMs={report.checkedAtMs + 12_000} {...extra} />);

export const Ready: StoryObj = { render: () => <ThemePair label="Doctor: four issues, one combined fix; the managed settings are listed as information with no control" stack>{page(DOCTOR_REPORT, "ready")}</ThemePair> };
export const ReadyProblems: StoryObj = { render: () => <ThemePair label="Doctor: problems Marshell will not fix say why, in words, and the headline says Nothing to fix" stack>{page(DOCTOR_PROBLEMS, "ready")}</ThemePair> };
export const AllClear: StoryObj = { render: () => <ThemePair label="Doctor: all clear, with only Run again" stack>{page(DOCTOR_CLEAR, "ready")}</ThemePair> };
export const Running: StoryObj = { render: () => <ThemePair label="Doctor: a check that takes over 150 ms says what it is checking; a quicker one shows nothing" stack>{page(DOCTOR_REPORT, "running", { elapsedMs: 400 })}</ThemePair> };
export const Failed: StoryObj = { render: () => <ThemePair label="Doctor: when it cannot run it says why and offers Run again" stack>{page(DOCTOR_REPORT, "failed", { failure: "The core is not running." })}</ThemePair> };

export const FixSheetOpen: StoryObj = { render: () => <ThemePair label="Fix sheet: the combined change grouped by file, the backup, how to undo and which sessions need a restart; focus starts on the title" stack>{frame(<FixSheet report={DOCTOR_REPORT} />, 0)}</ThemePair> };

const longBefore = Array.from({ length: 200 }, (_, i) => `    "rule-${i}": "allow",`).join("\n");
const longAfter = Array.from({ length: 200 }, (_, i) => `    "rule-${i}": "${i % 2 ? "allow" : "ask"}",`).join("\n");
const LONG: DoctorReport = {
  ...DOCTOR_REPORT,
  items: DOCTOR_REPORT.items.filter((i) => i.id === "claude-cleanup-period").map((i) => ({ ...i, restartSessions: undefined, diff: unifiedDiff(i.file!, `{\n${longBefore}\n}\n`, `{\n${longAfter}\n}\n`) })),
};
export const FixSheetLong: StoryObj = { render: () => <ThemePair label="Fix sheet: a 200-line change folds after 40 lines, with Show all" stack>{frame(<FixSheet report={LONG} />, 0)}</ThemePair> };

export const Fixed: StoryObj = { render: () => <ThemePair label="After the fix: each row says Fixed, and the receipt shows for 10 s with Restart them and Undo" stack>{page(DOCTOR_REPORT, "fixed", { undo: UNDO_FIXED, nowMs: UNDO_FIXED.fixedAtMs + 2_000 })}</ThemePair> };
export const FixedLater: StoryObj = { render: () => <ThemePair label="Later: the toast is gone and Undo last fix stays in the Doctor header" stack>{page(DOCTOR_REPORT, "fixed", { undo: UNDO_FIXED, nowMs: UNDO_FIXED.fixedAtMs + 60_000 })}</ThemePair> };
export const UndoStale: StoryObj = { render: () => <ThemePair label="A file changed since the fix: undo is withdrawn and the page says why and offers the differences" stack>{page(DOCTOR_REPORT, "fixed", { undo: { ...UNDO_FIXED, filesMatch: false }, nowMs: UNDO_FIXED.fixedAtMs + 60_000 })}</ThemePair> };

const RTL: DoctorReport = {
  ...DOCTOR_REPORT,
  items: DOCTOR_REPORT.items.map((i) => (i.id === "claude-cleanup-period" ? { ...i, title: "Claude מוחק תמלולים אחרי 30 יום", file: "C:\\Users\\רותם\\.claude\\settings.json" } : i)),
};
export const RtlFilePath: StoryObj = { render: () => <ThemePair label="A Hebrew title and a path with Hebrew in it stay readable and wrap" stack>{page(RTL, "ready")}</ThemePair> };
