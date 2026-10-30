import type { Ref } from "react";
import type { RepoScripts } from "../../generated/RepoScripts";
import { CommandList } from "../tasks/CommandList";
import { scriptLines } from "./confirmScripts";
import "./ScriptsTrust.css";

/**
 * The question a repo's `marshell.json` asks before any of it runs: the exact commands, in text. It uses the launcher's heading,
 * line and button classes, so it fits in the launcher's sheet and in a dialog without styles of its own. Neither button is the
 * default: focus goes to the heading (`headingRef`), so Enter cannot allow by reflex. `busy` keeps both buttons focusable and inert.
 */
export function ScriptsTrust({
  scripts,
  allowLabel,
  cancelLabel,
  onAllow,
  onCancel,
  busy = false,
  headingRef,
}: {
  scripts: RepoScripts;
  allowLabel: string;
  cancelLabel: string;
  onAllow: () => void;
  onCancel: () => void;
  busy?: boolean;
  headingRef?: Ref<HTMLHeadingElement>;
}) {
  return (
    <>
      <h3 ref={headingRef} tabIndex={-1} className="launcher__title">
        Allow this repo's scripts?
      </h3>
      <p className="launcher__lede" dir="auto">
        {`marshell.json in ${scripts.root} runs these commands on this computer:`}
      </p>
      <div className="scripts-trust__list" tabIndex={0} role="region" aria-label="Scripts">
        <CommandList commands={scriptLines(scripts)} />
      </div>
      <div className="launcher__actions">
        <button type="button" className="launcher__start" data-allow aria-disabled={busy || undefined} onClick={() => !busy && onAllow()}>
          {allowLabel}
        </button>
        <button type="button" className="launcher__back" aria-disabled={busy || undefined} onClick={() => !busy && onCancel()}>
          {cancelLabel}
        </button>
      </div>
    </>
  );
}
