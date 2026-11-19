import type { CSSProperties, KeyboardEvent } from "react";
import { ColorMenu } from "./ColorMenu";
import type { NoteTool } from "./editor";

/**
 * Whether a selection's end may show the bar: only once the editor is ready and the PDF is known to take notes (`blocked` is
 * undefined until `notesBlocked` answers), and never with the Highlight tool on, which highlights the selection itself.
 */
export function selectionBarShows(editorReady: boolean, tool: NoteTool, blocked: string | null | undefined): boolean {
  return editorReady && blocked === null && tool !== "highlight";
}

type Props = {
  /** The chosen highlight colour's value. */
  color: string;
  onColor: (value: string) => void;
  onHighlight: () => void;
  onComment: () => void;
  /** Esc in the bar: it goes and the keyboard returns to the pages. */
  onClose: () => void;
  /** Where it sits in the page's box. */
  style?: CSSProperties;
};

/**
 * The bar under a selection of the page's text: Highlight, the colour and Comment. A press on it leaves the selection as it is
 * (a button takes the focus without collapsing it, and the press is kept from starting a new one).
 */
export function SelectionBar({ color, onColor, onHighlight, onComment, onClose, style }: Props) {
  const onKeyDown = (e: KeyboardEvent) => {
    if (e.key !== "Escape" || e.shiftKey || e.ctrlKey || e.altKey || e.metaKey || e.nativeEvent.isComposing) return;
    e.preventDefault();
    onClose();
  };
  return (
    <div className="pdf-selbar" role="toolbar" aria-label="Selected text" style={style} onKeyDown={onKeyDown} onMouseDown={(e) => e.preventDefault()}>
      <button type="button" className="pdf-selbar__button" onClick={onHighlight}>
        Highlight
      </button>
      <ColorMenu value={color} onPick={onColor} />
      <button type="button" className="pdf-selbar__button" onClick={onComment}>
        Comment
      </button>
    </div>
  );
}
