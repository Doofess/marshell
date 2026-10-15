import type { Meta, StoryObj } from "@storybook/react-vite";
import { ThemePair } from "../../design/ThemePair";
import { unifiedDiff } from "../../lib/diff";
import { DiffView } from "./DiffView";

const meta: Meta = { title: "Diff view", parameters: { layout: "fullscreen" } };
export default meta;

const SETTINGS = "C:/Users/Home Office/.claude/settings.json";
const BEFORE = '{\n  "model": "opus",\n  "cleanupPeriodDays": 30,\n  "theme": "dark"\n}\n';
const AFTER = '{\n  "model": "opus",\n  "cleanupPeriodDays": 365,\n  "hooks": {\n    "Stop": [{ "command": "\\"C:/Users/Home Office/.marshell/bin/marshell-hook\\" event --agent claude --tag mshl1" }]\n  },\n  "theme": "dark"\n}\n';

const SMALL = unifiedDiff(SETTINGS, BEFORE, AFTER);
const LONG_BEFORE = Array.from({ length: 20 }, (_, i) => `  "key${i}": ${i},`).join("\n");
const LONG = unifiedDiff(SETTINGS, `{\n${LONG_BEFORE}\n}\n`, `{\n${Array.from({ length: 220 }, (_, i) => `  "entry${i}": ${i},`).join("\n")}\n}\n`);
const RTL = unifiedDiff("C:/Users/Home Office/.codex/config.toml", 'name = "בדיקה"\n', 'name = "בדיקה"\nlabel = "שלום עולם"\n');

const box = { maxInlineSize: 640, padding: 12 };
export const SmallChange: StoryObj = { render: () => <ThemePair label="A small change: every changed line has a sign and a hidden word as well as a fill" stack><div style={box}><DiffView diff={SMALL} label="Changes to settings.json" /></div></ThemePair> };
export const LongChange: StoryObj = { render: () => <ThemePair label="A long change folds after 40 lines" stack><div style={box}><DiffView diff={LONG} label="Changes to settings.json" /></div></ThemePair> };
export const Expanded: StoryObj = { render: () => <ThemePair label="Expanded: every line, no button" stack><div style={box}><DiffView diff={SMALL} label="Changes to settings.json" expanded /></div></ThemePair> };
export const NothingToChange: StoryObj = { render: () => <ThemePair label="Nothing to change" stack><div style={box}><DiffView diff="" label="Changes" /></div></ThemePair> };
export const RtlNames: StoryObj = { render: () => <ThemePair label="Text in another script reads in its own direction" stack><div style={box}><DiffView diff={RTL} label="Changes to config.toml" /></div></ThemePair> };
