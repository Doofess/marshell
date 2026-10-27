import { type Ref, useEffect, useId, useRef, useState } from "react";
import { DiffView } from "../../components/DiffView/DiffView";
import type { AgentInstall } from "../../generated/AgentInstall";
import type { AgentSettingsView } from "../../generated/AgentSettingsView";
import type { Endpoint } from "../../generated/Endpoint";
import type { FieldLock } from "../../generated/FieldLock";
import type { RawLock } from "../../generated/RawLock";
import type { SettingsChange } from "../../generated/SettingsChange";
import type { SettingsPlan } from "../../generated/SettingsPlan";
import type { SettingValue } from "../../generated/SettingValue";
import { getInstall, getSettingsView, planSettings, undoSettings } from "../../lib/api/accounts";
import { type ApplyOutcome, applyPreview, previewDiff } from "../../lib/api/agents";
import { ApiError } from "../../lib/api/client";
import { RawEditor } from "../settings/RawEditor";
import { ScopeBadge } from "../settings/ScopeBadge";
import type { CompanyAgent } from "../settings/settingsSections";
import { choiceOptions, draftValue, editSummary, formEdits, switchOptions, type Draft } from "./companyEdit";
import { afterApply, afterEdit, afterPlan, afterReload, afterUndo, announcement, fieldFocus, focusAfter, focusBack, isBusy, rawChange, rawDroppedLine, undoneLine, type PlanResult, type UndoResult } from "./companyFlow";
import { COMPANY_FIELDS, type CompanyField } from "./companyFields";
import { fileLine, MANUAL_UPDATE, newerLine, runningNote, versionLine } from "./companyText";
import "../settings/Settings.css";
import "./CompanySection.css";

export type CompanyMode = "form" | "raw";

/** Where the edit flow stands; the stateful wrapper owns it. */
export type CompanyReview = { kind: "editing" } | { kind: "planning" } | { kind: "review"; plan: SettingsPlan } | { kind: "applying" } | { kind: "saved"; snapshotId: string } | { kind: "undone" } | { kind: "undoing"; snapshotId: string } | { kind: "conflict" } | { kind: "error"; message: string };

export type CompanyViewProps = {
  agent: CompanyAgent;
  name: string;
  install: AgentInstall | null;
  view: AgentSettingsView | null;
  running: number;
  mode: CompanyMode;
  onMode: (mode: CompanyMode) => void;
  onUpdate: () => void;
  draft: Draft;
  onDraft: (key: string, input: string) => void;
  rawDraft: string | null;
  onRawDraft: (text: string) => void;
  /** Reload dropped the raw draft because the file's text changed. */
  rawDropped?: boolean;
  review: CompanyReview;
  onReview: () => void;
  onApply: () => void;
  onBack: () => void;
  onDiscard: () => void;
  onReload: () => void;
  onUndo: () => void;
  /** A control took focus; the container remembers which, so Reload can put focus back. */
  onFocusField?: (focusKey: string) => void;
  /** The root element, so the container can move focus inside it. */
  rootRef?: Ref<HTMLDivElement>;
};

const shown = (v: SettingValue | undefined) => v?.value ?? "–";

/** Why the raw file can't be edited as text. */
export const rawLockLine = (lock: RawLock) => {
  switch (lock) {
    case "not_plain_json":
      return "Raw editing is off: this file isn't plain JSON, and Marshell saves it as plain JSON.";
    case "repeated_key":
      return "Raw editing is off: a key appears twice in this file.";
    case "hidden":
      return "Raw editing is off: this file holds secret values, and they show as hidden here.";
  }
};

const lockLine = (lock: FieldLock, hasText: boolean) => {
  switch (lock.kind) {
    case "managed":
      return `Set by ${lock.path}, so Marshell can't change it here.`;
    case "not_scalar":
      return hasText ? "Set as a table in the file. Edit it in Raw file." : "Set as a table in the file.";
    case "hidden":
      return "Holds a value shown as hidden, so the form can't change it.";
    case "account":
      return "Antigravity keeps this with your Google account, not in this file. Change it with /config in Antigravity.";
  }
};

/** A button that stays focusable while busy: `aria-disabled` and a swallowed click, never `disabled`. */
function Btn({ children, busy = false, primary = false, focusKey, onClick }: { children: string; busy?: boolean; primary?: boolean; focusKey?: string; onClick: () => void }) {
  return (
    <button type="button" className="settings__btn" data-primary={primary || undefined} aria-disabled={busy || undefined} data-focus={focusKey} onClick={() => !busy && onClick()}>
      {children}
    </button>
  );
}

/** One curated row of an editable file: a control, or the value as today with the reason it is locked. */
function EditableRow({ field, v, input, error, hasText, onDraft, onFocusField }: { field: CompanyField; v: SettingValue | undefined; input: string; error: string | undefined; hasText: boolean; onDraft: (key: string, input: string) => void; onFocusField?: (focusKey: string) => void }) {
  const id = useId();
  const control = v && !v.lock ? v.control : undefined;
  const empty = control?.kind === "number" || control?.kind === "text";
  const describedBy = [`${id}-help`, error ? `${id}-error` : null].filter(Boolean).join(" ");
  const focusKey = fieldFocus(field.key);
  const focusProps = { "data-focus": focusKey, onFocus: () => onFocusField?.(focusKey) };
  const options = control?.kind === "choice" ? choiceOptions(control, input) : control?.kind === "switch" ? switchOptions(input) : null;
  return (
    <li className="field" data-locked={v?.lock ? "" : undefined}>
      <div className="field__head">
        {control ? (
          <label className="field__label" htmlFor={id}>
            {field.label}
          </label>
        ) : (
          <span className="field__label">{field.label}</span>
        )}
        {v?.value != null && <ScopeBadge scope={v.scope} />}
      </div>
      <p id={`${id}-help`} className="field__help" dir="auto">
        {empty ? `${field.help} Empty means not set.` : field.help}
      </p>
      {options && (
        <select id={id} className="field__control" {...focusProps} aria-describedby={describedBy} value={input} onChange={(e) => onDraft(field.key, e.target.value)}>
          {options.map((o) => (
            <option key={o.value} value={o.value}>
              {o.label}
            </option>
          ))}
        </select>
      )}
      {control?.kind === "number" && <input id={id} className="field__control" type="text" inputMode="numeric" {...focusProps} value={input} aria-describedby={describedBy} aria-invalid={error ? true : undefined} onChange={(e) => onDraft(field.key, e.target.value)} />}
      {control?.kind === "text" && <input id={id} className="field__control" type="text" {...focusProps} value={input} spellCheck={false} aria-describedby={describedBy} aria-invalid={error ? true : undefined} onChange={(e) => onDraft(field.key, e.target.value)} />}
      {error && (
        <p id={`${id}-error`} className="field__error">
          {error}
        </p>
      )}
      {!control && (
        <p className="company__value" dir="auto">
          {shown(v)}
        </p>
      )}
      {v?.lock && <p className="company__lock">{lockLine(v.lock, hasText)}</p>}
    </li>
  );
}

/** The manual-update sentence with its commands in `<code>`. */
function ManualUpdate({ agent, name }: { agent: CompanyAgent; name: string }) {
  const m = MANUAL_UPDATE[agent];
  return (
    <p className="company__line">
      {`Marshell can't tell how ${name} was installed. Update it with `}
      <code>{m.update}</code>
      {m.npm && (
        <>
          {", or "}
          <code>{m.npm}</code>
          {" for an npm install"}
        </>
      )}
      {"."}
    </p>
  );
}

/** One company CLI: version and Update, then its user settings file as a form or as text, editable when the core says so. Presentational. */
export function CompanyView({ agent, name, install, view, running, mode, onMode, onUpdate, draft, onDraft, rawDraft, onRawDraft, rawDropped = false, review, onReview, onApply, onBack, onDiscard, onReload, onUndo, onFocusField, rootRef }: CompanyViewProps) {
  const newer = install ? newerLine(install) : null;
  const note = runningNote(name, running);
  const file = view ? fileLine(view) : null;
  const byKey = new Map((view?.curated ?? []).map((v) => [v.key, v]));
  const editable = view?.editable === true;
  const { edits, errors } = view && editable ? formEdits(view.curated, draft) : { edits: [], errors: {} as Record<string, string> };
  const rawChanged = rawChange(mode, view, rawDraft) !== null;
  const changeCount = rawChanged ? 1 : edits.length;
  const blocked = !rawChanged && Object.keys(errors).length > 0;
  const inReview = review.kind === "review" || review.kind === "applying";
  const stripShown = editable && (review.kind === "editing" || review.kind === "planning" || review.kind === "error" || review.kind === "undone") && (changeCount > 0 || blocked);
  return (
    <div className="company" ref={rootRef}>
      {!install && !view && <p className="company__line">{`Reading ${name}…`}</p>}
      {install && (
        <div className="company__version">
          <p className="company__line company__line--strong">{versionLine(install)}</p>
          {newer && <p className="company__line">{newer}</p>}
          {install.update_command ? (
            <button type="button" className="settings__btn company__update" onClick={onUpdate}>
              {`Update ${name}`}
            </button>
          ) : (
            <ManualUpdate agent={agent} name={name} />
          )}
          {install.update_command && note && <p className="company__line">{note}</p>}
        </div>
      )}
      {view && (
        <>
          <p className="company__path" dir="auto">
            {view.path}
          </p>
          {file && <p className="company__line">{file}</p>}
          <div className="company__modes" role="radiogroup" aria-label="View">
            <label className="company__mode">
              <input type="radio" name={`company-view-${agent}`} value="form" checked={mode === "form"} onChange={() => onMode("form")} />
              <span>Form</span>
            </label>
            <label className="company__mode" data-disabled={view.raw === null || undefined}>
              <input type="radio" name={`company-view-${agent}`} value="raw" checked={mode === "raw"} disabled={view.raw === null} onChange={() => onMode("raw")} />
              <span>Raw file</span>
            </label>
          </div>
          {inReview ? (
            <section className="change-review" aria-label="Review of the changes" tabIndex={-1} data-focus="review">
              {review.kind === "review" && review.plan.normalized && <p className="change-bar__note">Shown as Marshell reads it: comments and layout are left out, and secret values show as hidden. The file keeps its comments and layout.</p>}
              {review.kind === "review" && <DiffView diff={previewDiff(review.plan.preview)} label={`Changes to ${view.path}`} />}
              <p className="change-bar__note">
                {"A backup is saved first, in "}
                <code>~/.marshell/snapshots/</code>
                {". To undo, Marshell puts that backup back, for as long as the file is unchanged since this change."}
              </p>
              <div className="change-bar__actions">
                <Btn primary busy={review.kind === "applying"} onClick={onApply}>
                  Apply
                </Btn>
                <Btn busy={review.kind === "applying"} onClick={onBack}>
                  Back to editing
                </Btn>
              </div>
            </section>
          ) : mode === "raw" && view.raw !== null ? (
            <div className="company__raw">
              {rawDropped && <p className="company__note">{rawDroppedLine(view.path)}</p>}
              {editable && view.text !== null && view.raw_lock === null ? (
                <textarea className="raw-editor__text" aria-label={view.path} data-focus="raw" onFocus={() => onFocusField?.("raw")} rows={Math.min(24, Math.max(6, (rawDraft ?? view.text).split("\n").length))} spellCheck={false} value={rawDraft ?? view.text} onChange={(e) => onRawDraft(e.target.value)} />
              ) : view.raw_lock === "not_plain_json" && view.text !== null ? (
                <textarea className="raw-editor__text" aria-label={view.path} rows={Math.min(24, Math.max(6, view.text.split("\n").length))} spellCheck={false} defaultValue={view.text} readOnly />
              ) : agent === "codex" ? (
                <textarea className="raw-editor__text" aria-label={view.path} rows={Math.min(24, Math.max(6, view.raw.split("\n").length))} spellCheck={false} defaultValue={view.raw} readOnly />
              ) : (
                <RawEditor path={view.path} text={view.raw} readOnly />
              )}
              {editable && view.text !== null && view.raw_lock === null ? (
                <p className="company__note">This is the file itself. Review shows the exact change before anything is written.</p>
              ) : view.raw_lock === "not_plain_json" && view.text !== null ? (
                <p className="company__note">{rawLockLine(view.raw_lock)}</p>
              ) : (
                <>
                  <p className="company__note">Shown as Marshell reads it: comments and layout are left out, and secret values show as hidden.</p>
                  {view.raw_lock !== null && <p className="company__note">{rawLockLine(view.raw_lock)}</p>}
                </>
              )}
            </div>
          ) : (
            <>
              <ul className="settings__fields">
                {COMPANY_FIELDS[agent].map((f) => {
                  const v = byKey.get(f.key);
                  if (editable) return <EditableRow key={f.key} field={f} v={v} input={draft[f.key] ?? (v ? draftValue(v) : "")} error={errors[f.key]} hasText={view.text !== null} onDraft={onDraft} onFocusField={onFocusField} />;
                  return (
                    <li key={f.key} className="field">
                      <div className="field__head">
                        <span className="field__label">{f.label}</span>
                        {v?.value != null && <ScopeBadge scope={v.scope} />}
                      </div>
                      <p className="field__help" dir="auto">
                        {f.help}
                      </p>
                      <p className="company__value" dir="auto">
                        {shown(v)}
                      </p>
                    </li>
                  );
                })}
              </ul>
              {view.others.length > 0 && (
                <>
                  <h3 className="company__others">Other settings in this file</h3>
                  <ul className="settings__fields">
                    {view.others.map((o) => (
                      <li key={o.key} className="field">
                        <div className="field__head">
                          <code className="company__key">{o.key}</code>
                          <ScopeBadge scope={o.scope} />
                        </div>
                        <p className="company__value" dir="auto">
                          {shown(o)}
                        </p>
                      </li>
                    ))}
                  </ul>
                </>
              )}
            </>
          )}
          {stripShown && (
            <section className="change-bar" aria-label="Pending changes">
              <p className="change-bar__summary">{rawChanged ? "Raw file changed" : editSummary(changeCount)}</p>
              {review.kind === "error" && (
                <p className="change-bar__error" tabIndex={-1} data-focus="result">
                  {review.message}
                </p>
              )}
              <div className="change-bar__actions">
                <Btn primary busy={blocked || review.kind === "planning"} focusKey="review-button" onClick={onReview}>
                  Review changes
                </Btn>
                <Btn busy={review.kind === "planning"} onClick={onDiscard}>
                  Discard
                </Btn>
              </div>
            </section>
          )}
          {(review.kind === "saved" || review.kind === "undoing") && (
            <div className="company__result" tabIndex={-1} data-focus="result">
              <p className="company__line">{`Saved. New ${name} sessions use it.`}</p>
              <Btn busy={review.kind === "undoing"} onClick={onUndo}>
                Undo
              </Btn>
            </div>
          )}
          {review.kind === "undone" && (
            <div className="company__result" tabIndex={-1} data-focus="result">
              <p className="company__line">{undoneLine(view.path)}</p>
            </div>
          )}
          {review.kind === "error" && !stripShown && (
            <div className="company__result" tabIndex={-1} data-focus="result">
              <p className="change-bar__error">{review.message}</p>
            </div>
          )}
          {review.kind === "conflict" && (
            <div className="company__result" tabIndex={-1} data-focus="result">
              <p className="company__line">{`${view.path} changed since you opened it. Reload to see it.`}</p>
              <Btn onClick={onReload}>Reload</Btn>
            </div>
          )}
        </>
      )}
    </div>
  );
}

/** Fetches the CLI's install and settings view when it opens and again, with a fresh version read, whenever `refresh` changes. Owns the edit flow. */
export function CompanySection({ endpoint, agent, name, running, onUpdate, refresh, announce }: { endpoint: Endpoint; agent: CompanyAgent; name: string; running: number; onUpdate: (agent: CompanyAgent) => void; refresh: number; announce: (text: string) => void }) {
  const [install, setInstall] = useState<AgentInstall | null>(null);
  const [view, setView] = useState<AgentSettingsView | null>(null);
  const [failed, setFailed] = useState(false);
  const [mode, setMode] = useState<CompanyMode>("form");
  const [draft, setDraft] = useState<Draft>({});
  const [rawDraft, setRawDraft] = useState<string | null>(null);
  const [rawDropped, setRawDropped] = useState(false);
  const [review, setReview] = useState<CompanyReview>({ kind: "editing" });
  // Each plan, apply or undo takes a number; a result whose number is no longer current (Discard, Back, a newer request, unmount) is dropped.
  const request = useRef(0);
  useEffect(
    () => () => {
      request.current += 1;
    },
    [],
  );
  // The focused button goes away with each step, so focus moves to what replaced it (see focusAfter).
  const root = useRef<HTMLDivElement>(null);
  const lastEdited = useRef<string | null>(null);
  const lastFocused = useRef<string | null>(null);
  const [pendingFocus, setPendingFocus] = useState<string | null>(null);
  // Discard and Reload change what is on screen, so the control is focused after that render; no match leaves focus alone.
  useEffect(() => {
    if (pendingFocus === null) return;
    root.current?.querySelector<HTMLElement>(`[data-focus="${pendingFocus}"]`)?.focus();
    setPendingFocus(null);
  }, [pendingFocus]);
  const shownKind = useRef(review.kind);
  useEffect(() => {
    const target = focusAfter(shownKind.current, review.kind);
    shownKind.current = review.kind;
    if (target) root.current?.querySelector<HTMLElement>(`[data-focus="${target}"]`)?.focus();
  }, [review.kind]);
  useEffect(() => {
    let live = true;
    void (async () => {
      try {
        const [i, v] = await Promise.all([getInstall(endpoint, agent, refresh > 0), getSettingsView(endpoint, agent)]);
        if (!live) return;
        setInstall(i);
        setView(v);
        setFailed(false);
      } catch {
        if (live) setFailed(true);
      }
    })();
    return () => {
      live = false;
    };
  }, [endpoint, agent, refresh]);
  const path = view?.path ?? "";
  const shownMode: CompanyMode = view?.raw === null ? "form" : mode;
  const land = (next: CompanyReview) => {
    setReview(next);
    const text = announcement(next, name, path);
    if (text) announce(text);
  };
  const reread = async () => {
    try {
      setView(await getSettingsView(endpoint, agent));
    } catch {
      // The view stays as it was; Reload reads again.
    }
  };
  const onDraft = (key: string, input: string) => {
    if (isBusy(review)) return;
    lastEdited.current = fieldFocus(key);
    setDraft((d) => ({ ...d, [key]: input }));
    setReview((r) => afterEdit(r));
  };
  const onRawDraft = (text: string) => {
    if (isBusy(review)) return;
    lastEdited.current = "raw";
    setRawDraft(text);
    setRawDropped(false);
    setReview((r) => afterEdit(r));
  };
  const onReview = async () => {
    if (!view || isBusy(review)) return;
    const mine = ++request.current;
    const raw = rawChange(shownMode, view, rawDraft);
    const change: SettingsChange = raw !== null ? { kind: "raw", text: raw } : { kind: "form", edits: formEdits(view.curated, draft).edits };
    setReview({ kind: "planning" });
    let result: PlanResult;
    try {
      result = { ok: true, plan: await planSettings(endpoint, agent, { expected_hash: view.hash ?? undefined, change }) };
    } catch (e) {
      result = { ok: false, status: e instanceof ApiError ? e.status : 0, message: e instanceof Error ? e.message : String(e) };
    }
    if (request.current !== mine) return;
    land(afterPlan(result));
  };
  const onApply = async () => {
    if (review.kind !== "review") return;
    const mine = ++request.current;
    setReview({ kind: "applying" });
    let outcome: ApplyOutcome;
    try {
      outcome = await applyPreview(endpoint, review.plan.preview);
    } catch (e) {
      outcome = { ok: false, conflict: false, message: e instanceof Error ? e.message : String(e) };
    }
    if (request.current !== mine) return;
    if (outcome.ok) {
      await reread();
      if (request.current !== mine) return;
      setDraft({});
      setRawDraft(null);
    }
    land(afterApply(outcome));
  };
  const onBack = () => {
    if (review.kind !== "review") return;
    request.current += 1;
    setReview({ kind: "editing" });
  };
  const onDiscard = () => {
    if (isBusy(review)) return;
    request.current += 1;
    setDraft({});
    setRawDraft(null);
    setRawDropped(false);
    setReview({ kind: "editing" });
    setPendingFocus(focusBack("discard", lastEdited.current, lastFocused.current));
    lastEdited.current = null;
  };
  const onReload = async () => {
    if (isBusy(review)) return;
    const mine = ++request.current;
    try {
      const v = await getSettingsView(endpoint, agent);
      if (request.current !== mine) return;
      const kept = afterReload(view?.text ?? null, v, draft, rawDraft);
      setView(v);
      setDraft(kept.draft);
      setRawDraft(kept.rawDraft);
      if (kept.rawDropped) {
        setRawDropped(true);
        announce(rawDroppedLine(v.path));
      }
      setReview({ kind: "editing" });
      setPendingFocus(focusBack("reload", lastEdited.current, lastFocused.current));
    } catch {
      if (request.current === mine) land({ kind: "error", message: `Couldn't read ${path} again. Close Settings and try again.` });
    }
  };
  const onUndo = async () => {
    if (review.kind !== "saved") return;
    const mine = ++request.current;
    const snapshotId = review.snapshotId;
    setReview({ kind: "undoing", snapshotId });
    let result: UndoResult;
    try {
      await undoSettings(endpoint, agent, snapshotId);
      result = { ok: true };
    } catch (e) {
      result = { ok: false, status: e instanceof ApiError ? e.status : 0, message: e instanceof Error ? e.message : String(e) };
    }
    if (request.current !== mine) return;
    if (!result.ok) {
      land(afterUndo(result, path));
      return;
    }
    await reread();
    if (request.current !== mine) return;
    setDraft({});
    setRawDraft(null);
    land(afterUndo(result, path));
  };
  return (
    <>
      {failed && <p className="company__error">{`Couldn't read ${name} from Marshell's core. Close Settings and try again.`}</p>}
      {(!failed || install || view) && (
        <CompanyView
          agent={agent}
          name={name}
          install={install}
          view={view}
          running={running}
          mode={shownMode}
          onMode={setMode}
          onUpdate={() => onUpdate(agent)}
          draft={draft}
          onDraft={onDraft}
          rawDraft={rawDraft}
          onRawDraft={onRawDraft}
          rawDropped={rawDropped}
          review={review}
          onReview={() => void onReview()}
          onApply={() => void onApply()}
          onBack={onBack}
          onDiscard={onDiscard}
          onReload={() => void onReload()}
          onUndo={() => void onUndo()}
          onFocusField={(k) => (lastFocused.current = k)}
          rootRef={root}
        />
      )}
    </>
  );
}
