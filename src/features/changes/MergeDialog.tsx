import { useState } from "react";
import type { MergeMethod } from "../../generated/MergeMethod";
import { failureOf, mergePull } from "./githubApi";
import { METHOD_BUTTONS, githubCopy as gc } from "./githubCopy";
import { Notice, SheetButton, SheetFrame } from "./PullSheetParts";

/**
 * "Merge #182?": confirms the pull request, the method and the base. A failure shows GitHub's sentence here and keeps both buttons.
 * Merging now is said by the bar when its reply changes; auto-merge changes no reply the bar words, so this dialog says it.
 */
export function MergeDialog(p: {
  tab: string;
  checkout: string;
  pull: { id: string; number: number; title: string; head: string; base: string };
  method: MergeMethod;
  auto: boolean;
  announce(text: string): void;
  onClose: () => void;
  onDone: () => void;
  initialError?: string;
  initialBusy?: boolean;
}) {
  const [busy, setBusy] = useState(p.initialBusy ?? false);
  const [error, setError] = useState<string | null>(p.initialError ?? null);
  const go = async () => {
    if (busy) return;
    setBusy(true);
    setError(null);
    try {
      await mergePull(p.tab, { checkout: p.checkout, id: p.pull.id, number: p.pull.number, head: p.pull.head, method: p.method, auto: p.auto });
    } catch (e) {
      const reason = failureOf(e).reason;
      setBusy(false);
      setError(reason);
      p.announce(reason);
      return;
    }
    if (p.auto) p.announce(gc.mergesLater(p.pull.number));
    p.onDone();
  };
  return (
    <SheetFrame
      title={gc.mergeTitle(p.pull.number)}
      closable={!busy}
      onClose={p.onClose}
      focusKey={error ?? ""}
      foot={
        <>
          <SheetButton label={gc.cancel} onPress={p.onClose} off={busy} />
          <SheetButton label={p.auto ? gc.mergeWhenChecksPass : METHOD_BUTTONS[p.method]} onPress={() => void go()} primary off={busy} />
        </>
      }
    >
      <p className="close-dialog__line">{gc.mergeLine(METHOD_BUTTONS[p.method], p.pull.title, p.pull.number, p.pull.base)}</p>
      {p.auto && <p className="close-dialog__line">{gc.mergeAuto}</p>}
      {error !== null && <Notice text={error} alert />}
      {busy && <p className="close-dialog__line">{gc.merging}</p>}
    </SheetFrame>
  );
}
