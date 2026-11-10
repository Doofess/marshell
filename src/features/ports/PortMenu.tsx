import { ContextMenu } from "../../components/ContextMenu/ContextMenu";
import { useOpenPageInPane } from "../pane/PaneContext";
import type { PortMenuRequest } from "./PortChips";
import { chipLabel, chipMenuItems, runChipItem } from "./ports";

/**
 * A port chip's menu. Where the side pane has browser tabs, "Open" opens one there and the menu gains the way out to the system
 * browser. `onDone` ends the menu; focus goes back to the chip that asked for it.
 */
export function PortMenu({ request, onDone }: { request: PortMenuRequest; onDone: () => void }) {
  const openPage = useOpenPageInPane();
  return (
    <ContextMenu
      label={request.chips.length === 1 ? `Port ${chipLabel(request.chips[0]!)}` : "More ports"}
      items={chipMenuItems(request.chips, openPage !== null)}
      anchor={request.anchor}
      onRun={(id) => {
        runChipItem(request.chips, id, openPage);
        onDone();
        request.opener?.focus();
      }}
      onClose={({ refocus }) => {
        onDone();
        if (refocus) request.opener?.focus();
      }}
    />
  );
}
