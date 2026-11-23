import { type ReactNode, useEffect, useId, useRef } from "react";
import type { PushPlan } from "../../generated/PushPlan";
import { useDialogFocus } from "../../lib/useDialogFocus";
import { useCoverPage } from "../browser/pageCover";
import "../window/CloseDialog.css";
import { githubCopy as gc } from "./githubCopy";
import { type RunState, shouldFollow } from "./pushRun";
import "./PullSheets.css";

/**
 * The shell the three sheets share: the close dialog's classes, a title that takes focus on open, and one scrolling body above
 * the buttons. After the first render, a change of `focusKey` moves focus to the body's `[data-focus]` element (the sentence
 * or the output a state change is about). `closable` is false while a push runs, or a pull request is being made or merged:
 * Esc and a press outside then do nothing and the sheet's own buttons are the way out.
 */
export function SheetFrame(p: {
  title: string;
  closable: boolean;
  onClose: () => void;
  focusKey: string;
  /** A key that changes when the output grows, so the body follows its last line unless the reader scrolled up. */
  followKey?: number;
  /** Makes the body keyboard-scrollable and names it. */
  bodyLabel?: string;
  /** The body itself is what a state change is about (the output of a running push), so focus moves to it. */
  bodyFocus?: boolean;
  children: ReactNode;
  foot: ReactNode;
}) {
  const dialog = useRef<HTMLDivElement>(null);
  const heading = useRef<HTMLHeadingElement>(null);
  const body = useRef<HTMLDivElement>(null);
  const follow = useRef(true);
  const first = useRef(true);
  const title = useId();
  useCoverPage();
  useDialogFocus(dialog, heading, () => p.closable && p.onClose());
  useEffect(() => {
    if (first.current) {
      first.current = false;
      return;
    }
    dialog.current?.querySelector<HTMLElement>("[data-focus]")?.focus();
  }, [p.focusKey]);
  useEffect(() => {
    const el = body.current;
    if (el && follow.current) el.scrollTop = el.scrollHeight;
  }, [p.followKey]);
  const onScroll = () => {
    const el = body.current;
    if (el) follow.current = shouldFollow(el.scrollTop, el.clientHeight, el.scrollHeight);
  };
  return (
    <div className="close-dialog">
      <div className="close-dialog__backdrop" onClick={() => p.closable && p.onClose()} />
      <div ref={dialog} className="close-dialog__sheet pull-sheet" role="dialog" aria-modal="true" aria-labelledby={title}>
        <h2 ref={heading} id={title} tabIndex={-1} className="close-dialog__title">
          {p.title}
        </h2>
        <div ref={body} className="pull-sheet__body" onScroll={onScroll} tabIndex={p.bodyLabel ? 0 : undefined} aria-label={p.bodyLabel} data-focus={p.bodyFocus ? "" : undefined}>
          {p.children}
        </div>
        <footer className="close-dialog__foot pull-sheet__foot">{p.foot}</footer>
      </div>
    </div>
  );
}

/** A sheet's button: `aria-disabled` keeps it in the tab order, and the handler ignores a press while it is off. */
export function SheetButton(p: { label: string; onPress: () => void; primary?: boolean; off?: boolean }) {
  return (
    <button type="button" className="close-dialog__btn" data-default={p.primary || undefined} aria-disabled={p.off || undefined} onClick={() => !p.off && p.onPress()}>
      {p.label}
    </button>
  );
}

/** A sentence a state change is about: focus moves to it. */
export function Notice(p: { text: string; alert?: boolean }) {
  return (
    <p className="close-dialog__line pull-sheet__notice" tabIndex={-1} data-focus="" data-problem={p.alert || undefined}>
      {p.text}
    </p>
  );
}

/** The folder, what would be pushed and what stays out. `head` is the line under the folder. */
export function PlanBody({ plan, head, showCount }: { plan: PushPlan; head: string; showCount: boolean }) {
  return (
    <>
      <p className="pull-sheet__folder">{plan.checkout}</p>
      <p className="close-dialog__line">{head}</p>
      {showCount && plan.to_push > 0 && <p className="close-dialog__line">{gc.willPush(plan.to_push)}</p>}
      {plan.commits.length > 0 && (
        <ul className="pull-sheet__commits">
          {plan.commits.map((c) => (
            <li key={c.id} className="pull-sheet__commit">
              <code className="pull-sheet__id">{c.id}</code> <span>{c.subject}</span>
            </li>
          ))}
        </ul>
      )}
      {plan.more > 0 && <p className="close-dialog__line">{gc.andMoreCommits(plan.more)}</p>}
      {plan.uncommitted > 0 && <p className="close-dialog__line">{gc.uncommitted(plan.uncommitted)}</p>}
      {plan.moved && <p className="close-dialog__line">{gc.moved}</p>}
    </>
  );
}

/** What the push shows once it is under way: git's output as text, never as HTML. */
export function Output({ lines }: { lines: string[] }) {
  return lines.length > 0 ? <pre className="pull-sheet__output">{lines.join("\n")}</pre> : null;
}

/** The views every push flow shares, for the states `running` to `cancelled`; null for any other. */
export function runView(s: RunState, act: { close: () => void; cancel: () => void; tryAgain: () => void }): { body: ReactNode; foot: ReactNode; label?: string; focus?: boolean } | null {
  // Output that is kept after an end stays within the keyboard's reach; focus is on the end's sentence then.
  const kept = s.lines.length > 0 ? gc.outputLabel : undefined;
  switch (s.kind) {
    case "running":
    case "pushed":
      return {
        label: gc.outputLabel,
        focus: true,
        body: (
          <>
            <p className="close-dialog__line">{s.plan ? gc.pushingTo(s.plan.branch) : gc.pushingOrigin}</p>
            <Output lines={s.lines} />
          </>
        ),
        foot: s.kind === "running" ? <SheetButton label={gc.cancel} onPress={act.cancel} /> : null,
      };
    case "failed":
      return {
        label: kept,
        body: (
          <>
            <Notice text={s.reason ?? ""} alert />
            <Output lines={s.lines} />
          </>
        ),
        foot: (
          <>
            <SheetButton label={gc.close} onPress={act.close} />
            {s.plan !== null && <SheetButton label={gc.tryAgain} onPress={act.tryAgain} primary />}
          </>
        ),
      };
    case "cancelled":
      return {
        label: kept,
        body: (
          <>
            <Notice text={gc.stopped} />
            <Output lines={s.lines} />
          </>
        ),
        foot: <SheetButton label={gc.close} onPress={act.close} />,
      };
    default:
      return null;
  }
}

/** The views before anything is confirmed: the plan being read, or refused. */
export function startView(s: RunState, close: () => void): { body: ReactNode; foot: ReactNode } | null {
  if (s.kind === "loading") return { body: <p className="close-dialog__line">{gc.readingCheckout}</p>, foot: <SheetButton label={gc.cancel} onPress={close} /> };
  if (s.kind === "refused") return { body: <Notice text={s.reason ?? ""} />, foot: <SheetButton label={gc.close} onPress={close} /> };
  return null;
}
