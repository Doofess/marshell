import { useRef, useState } from "react";
import { githubCopy as gc } from "./githubCopy";
import { Notice, PlanBody, SheetButton, SheetFrame, runView, startView } from "./PullSheetParts";
import type { RunState } from "./pushRun";
import { type CreateFields, usePushRun } from "./usePushRun";

const TITLE_MAX = 256;
const BODY_MAX = 65_536;

/** Whether the fields can be sent, and the sentence for a problem the person can see (an empty title just keeps the button off). */
export function createProblem(f: CreateFields): { off: boolean; message: string | null } {
  const title = f.title.trim().length;
  if (title > TITLE_MAX) return { off: true, message: gc.titleLength };
  if (f.body.length > BODY_MAX) return { off: true, message: gc.bodyLength };
  if (f.base.trim() === "") return { off: true, message: gc.baseEmpty };
  return { off: title === 0, message: null };
}

/**
 * "Create pull request": the checkout's plan, the fields, then the push (when there is something to push) and the pull request.
 * It announces nothing when it succeeds: the panel says "Pull request #N was created." when the bar's reply changes.
 * `repo` is "owner/name" from the bar.
 */
export function CreatePullSheet(p: { tab: string; repo: string; announce(text: string): void; onClose: () => void; onDone: () => void; initial?: RunState }) {
  const [edits, setEdits] = useState<Partial<CreateFields>>({});
  const fieldsRef = useRef<() => CreateFields>(() => ({ title: "", body: "", base: "", draft: false }));
  const run = usePushRun({ tab: p.tab, create: true, announce: p.announce, onDone: p.onDone, initial: p.initial, fields: () => fieldsRef.current() });
  const s = run.state;
  const plan = s.plan;
  const fields: CreateFields = { title: edits.title ?? plan?.title ?? "", body: edits.body ?? plan?.body ?? "", base: edits.base ?? plan?.base ?? "", draft: edits.draft ?? false };
  fieldsRef.current = () => fields;
  const problem = createProblem(fields);
  const set = (o: Partial<CreateFields>) => setEdits((e) => ({ ...e, ...o }));
  const creating = s.kind === "creating" || s.kind === "created";

  const ran = runView(s, { close: p.onClose, cancel: run.cancel, tryAgain: run.tryAgain });
  const view = startView(s, p.onClose) ?? ran;
  let shown: { body: React.ReactNode; foot: React.ReactNode } | null = view;
  if (!shown && plan && (s.kind === "confirm" || creating || s.kind === "create_failed")) {
    const label = plan.to_push > 0 && s.kind !== "create_failed" ? gc.pushAndCreate : gc.createNow;
    shown = {
      body: (
        <>
          <PlanBody plan={plan} head={gc.headLine(plan.branch, fields.base.trim() || plan.base, p.repo)} showCount={s.kind !== "create_failed"} />
          {s.kind === "create_failed" && <Notice text={s.reason ?? ""} alert />}
          <label className="pull-sheet__field">
            <span>{gc.titleField}</span>
            <input type="text" className="pull-sheet__input" value={fields.title} readOnly={creating} onChange={(e) => set({ title: e.target.value })} />
          </label>
          <label className="pull-sheet__field">
            <span>{gc.bodyField}</span>
            <textarea className="pull-sheet__input pull-sheet__text" rows={6} value={fields.body} readOnly={creating} onChange={(e) => set({ body: e.target.value })} />
          </label>
          <label className="pull-sheet__field">
            <span>{gc.baseField}</span>
            <input type="text" className="pull-sheet__input" value={fields.base} readOnly={creating} onChange={(e) => set({ base: e.target.value })} />
          </label>
          <label className="pull-sheet__check">
            <input type="checkbox" checked={fields.draft} disabled={creating} onChange={(e) => set({ draft: e.target.checked })} />
            <span>{gc.draftField}</span>
          </label>
          {problem.message && <p className="close-dialog__error">{problem.message}</p>}
          {creating && <Notice text={gc.creating} />}
        </>
      ),
      foot: (
        <>
          <SheetButton label={gc.cancel} onPress={p.onClose} off={creating} />
          <SheetButton label={label} onPress={run.confirm} primary off={problem.off || creating} />
        </>
      ),
    };
  }
  return (
    <SheetFrame
      title={gc.createTitle}
      closable={s.kind !== "running" && s.kind !== "pushed" && !creating}
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
