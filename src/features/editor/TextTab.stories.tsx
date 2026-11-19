import type { Meta, StoryObj } from "@storybook/react-vite";
import { ThemePair } from "../../design/ThemePair";
import type { Draft } from "../../generated/Draft";
import "../preview/Preview.css";
import { SaveDialog } from "./SaveDialog";
import { start, step, type DocEvent, type DocState } from "./editorDoc";
import { TextTabView, type TextTabViewProps } from "./TextTab";

// Nothing here calls the core: each state is made with the document reducer, and the view takes it as a prop.
const meta: Meta = { title: "Text tab", parameters: { layout: "fullscreen" } };
export default meta;

const NAME = "notes.txt";
const PATH = "C:\\work\\notes.txt";
const H1 = "1".repeat(64);
const H2 = "2".repeat(64);
const TEXT = "Buy milk\nCall the bank\nBook the train";

/** The state after the events, from a clean open of TEXT. */
function after(...events: DocEvent[]): DocState {
  let s = step(start, { type: "opened", disk: { text: TEXT, hash: H1 }, draft: null }, NAME).state;
  for (const e of events) s = step(s, e, NAME).state;
  return s;
}

const noop = () => {};
const props = (doc: DocState, over: Partial<TextTabViewProps> = {}): TextTabViewProps => ({
  entry: { path: PATH, line: null, col: null, view: "source" },
  name: NAME,
  kind: "text",
  doc,
  ready: true,
  gone: false,
  readOnly: null,
  encoding: "utf-8",
  eol: "lf",
  cursor: { line: 1, column: 1 },
  wrap: true,
  hintId: "text-tab-story-hint",
  editor: null,
  onEdit: noop,
  onCursor: noop,
  onMerge: noop,
  onConflict: noop,
  onKeep: noop,
  onTake: noop,
  onUndo: noop,
  onLink: noop,
  onKey: noop,
  ...over,
});

const framed = (label: string, doc: DocState, over: Partial<TextTabViewProps> = {}): StoryObj => ({
  render: () => (
    <ThemePair label={label} stack>
      <div style={{ height: "24rem", display: "flex", flexDirection: "column" }}>
        <TextTabView {...props(doc, over)} />
      </div>
    </ThemePair>
  ),
});

const draft: Draft = { path: PATH, text: "Buy oat milk\nCall the bank\nBook the train", base_text: TEXT, base_hash: H1, encoding: "utf-8", eol: "lf", edited_ms: 1 };

export const Clean: StoryObj = framed("A text file open and clean: the editor, and the footer under it", after());
export const ChangedOnDisk: StoryObj = framed("The file changed on disk while the tab holds unsaved edits: the bar offers Merge, Keep mine and Take disk", step(start, { type: "opened", disk: { text: "Buy milk\nCall the bank\nBook the plane", hash: H2 }, draft }, NAME).state);
export const Merged: StoryObj = framed(
  "After Merge: the text holds one conflict, and the line under it says so",
  (() => {
    let s = step(start, { type: "opened", disk: { text: "Buy milk\nCall the bank\nBook the plane", hash: H2 }, draft }, NAME).state;
    s = step(s, { type: "merge" }, NAME).state;
    const text = ["Buy oat milk", "Call the bank", "<<<<<<< mine", "Book the train", "=======", "Book the plane", ">>>>>>> disk"].join("\n");
    return step(s, { type: "merged", text, conflicts: 1, mine: draft.text, disk: H2 }, NAME).state;
  })(),
);
export const SavedWithUndo: StoryObj = framed("After Ctrl+S: Saved, with an Undo button under the text", after({ type: "edited", text: "Buy oat milk" }, { type: "save" }, { type: "saved", hash: H2, snapshot: "snap" }));
export const ReadOnly: StoryObj = framed("A file with the read-only attribute: its line says so", after(), { readOnly: "attribute" });
// The dialog covers the pane, so the story gives it a frame of its own to sit in.
export const SaveChanges: StoryObj = {
  render: () => (
    <ThemePair label="Closing a tab with unsaved edits asks first: Save has the focus" stack>
      <div style={{ height: "24rem", position: "relative", overflow: "hidden" }}>
        <SaveDialog name={NAME} onChoose={noop} />
      </div>
    </ThemePair>
  ),
};
