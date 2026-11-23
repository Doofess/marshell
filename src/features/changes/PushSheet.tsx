import { githubCopy as gc } from "./githubCopy";
import { PlanBody, SheetButton, SheetFrame, runView, startView } from "./PullSheetParts";
import { INITIAL_RUN, type RunState } from "./pushRun";
import { usePushRun } from "./usePushRun";

/**
 * "Push to GitHub": confirms what would be pushed, then shows git's output while it runs. `resume` opens it on a push that is
 * already running (Show output), with no plan call. `onDone` runs after a successful push, which this sheet announces.
 */
export function PushSheet(p: { tab: string; announce(text: string): void; onClose: () => void; onDone: () => void; initial?: RunState; resume?: string }) {
  const initial = p.initial ?? (p.resume ? { ...INITIAL_RUN, kind: "running" as const, id: p.resume } : undefined);
  const run = usePushRun({ tab: p.tab, create: false, announce: p.announce, onDone: p.onDone, initial });
  const s = run.state;
  const ran = runView(s, { close: p.onClose, cancel: run.cancel, tryAgain: run.tryAgain });
  const view = startView(s, p.onClose) ?? ran;
  const confirm =
    s.kind === "confirm" && s.plan
      ? {
          body: <PlanBody plan={s.plan} head={gc.pushHead(s.plan.branch)} showCount />,
          foot: (
            <>
              <SheetButton label={gc.cancel} onPress={p.onClose} />
              <SheetButton label={gc.pushButton(s.plan.to_push)} onPress={run.confirm} primary />
            </>
          ),
        }
      : null;
  const shown = view ?? confirm;
  return (
    <SheetFrame
      title={gc.pushTitle}
      closable={s.kind !== "running" && s.kind !== "pushed"}
      onClose={p.onClose}
      focusKey={s.kind}
      followKey={s.lines.length}
      bodyLabel={ran?.label}
      bodyFocus={ran?.focus}
      foot={shown?.foot}
    >
      {shown?.body}
    </SheetFrame>
  );
}
