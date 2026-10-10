import { useId, type KeyboardEvent } from "react";
import { AGENT_NAMES } from "../../components/AgentMark/agents";
import { AuxGlyph, StatusGlyph } from "../../components/StatusGlyph/StatusGlyph";
import { formatDuration } from "../sidebar/format";
import { keyAction, type CardAction } from "./arming";
import { headline, receiptText, subject } from "./headline";
import { isTruncated, payloadLines, showAllLabel } from "./payload";
import type { ApprovalRequest } from "./types";
import "./ApproveCard.css";

export type ApproveCardProps = {
  request: ApprovalRequest;
  /** Payload shown in full (the user pressed Space or the card is in the drawer). */
  expanded?: boolean;
  /** False during the 500 ms after focus or a content change. Defaults to true so static stories show live buttons. */
  armed?: boolean;
  /** 0..1, drawn on the risky Allow button's ring. */
  holdProgress?: number;
  /** One-shot: the receipt is leaving (140 ms). */
  exiting?: boolean;
  onAction?: (a: CardAction | `answer:${number}`) => void;
  onHoldEnd?: () => void;
  onFocus?: () => void;
};

const RING_R = 6;
const RING_C = 2 * Math.PI * RING_R;

function Ring({ progress }: { progress: number }) {
  return (
    <svg className="approve-card__ring" width={16} height={16} viewBox="0 0 16 16" aria-hidden="true">
      <circle className="approve-card__ring-track" cx="8" cy="8" r={RING_R} fill="none" strokeWidth="2" />
      <circle
        className="approve-card__ring-value"
        cx="8"
        cy="8"
        r={RING_R}
        fill="none"
        strokeWidth="2"
        strokeDasharray={`${RING_C * progress} ${RING_C}`}
        transform="rotate(-90 8 8)"
      />
    </svg>
  );
}

const Kbd = ({ k }: { k: string }) => (
  <kbd className="approve-card__kbd" aria-hidden="true">
    {k}
  </kbd>
);

/**
 * The approve card (docs/PLAN.md "The approve card"). Presentational: the parent owns the request, the arming timer
 * (useArming) and the hold timer (useHold). Keys act only while the card has focus; Enter never allows.
 */
export function ApproveCard({ request, expanded = false, armed = true, holdProgress = 0, exiting = false, onAction, onHoldEnd, onFocus }: ApproveCardProps) {
  const riskId = useId();
  const { detail, risk, state, session } = request;
  const agent = AGENT_NAMES[session.agent];

  if (state.phase === "decided") {
    return (
      <div className="approve-card approve-card--receipt" role="status" data-agent={session.agent} data-phase="decided" data-verdict={state.verdict} data-exiting={exiting || undefined}>
        <span aria-hidden="true">
          <StatusGlyph kind={state.verdict === "allowed" ? "done-seen" : "error"} size={12} />
        </span>
        <span className="approve-card__receipt-text" dir="auto">
          {receiptText(detail, state.verdict)}
        </span>
      </div>
    );
  }
  if (state.phase === "terminal") {
    return (
      <div className="approve-card approve-card--receipt" role="status" data-agent={session.agent} data-phase="terminal">
        <span className="approve-card__receipt-text" dir="auto">
          Answered in terminal {"·"} {subject(detail)}
        </span>
      </div>
    );
  }
  if (state.phase === "released") {
    return (
      <div className="approve-card approve-card--receipt" role="status" data-agent={session.agent} data-phase="released">
        <span className="approve-card__receipt-text">
          Timed out after {formatDuration(state.afterMs)}. {agent} is asking in its own terminal now.
        </span>
      </div>
    );
  }

  const lines = payloadLines(detail);
  const truncated = isTruncated(lines);
  const collapsed = truncated && !expanded;
  const risky = risk.level === "risky";
  const isQuestion = detail.kind === "question";
  const cwd = detail.kind === "bash" && detail.cwd && detail.cwd !== request.sessionCwd ? detail.cwd : null;
  const ctx = { risky, armed, hasAlways: Boolean(request.alwaysRule), truncated, expanded };

  const fire = (a: CardAction | `answer:${number}`) => {
    if (!armed && a !== "jump" && a !== "expand") return;
    onAction?.(a);
  };
  const onKeyDown = (e: KeyboardEvent) => {
    if (e.ctrlKey || e.metaKey || e.altKey || e.repeat) return;
    const a = keyAction(e.key, ctx);
    if (!a) return;
    e.preventDefault();
    onAction?.(a);
  };
  const onKeyUp = (e: KeyboardEvent) => {
    if (risky && e.key.toLowerCase() === "y") onHoldEnd?.();
  };

  const disabled = armed ? undefined : true;

  return (
    <div
      className="approve-card"
      role="group"
      aria-label={`${session.name}: ${headline(detail)}`}
      tabIndex={0}
      data-agent={session.agent}
      data-phase="pending"
      data-risk={risk.level}
      data-armed={armed}
      onKeyDown={onKeyDown}
      onKeyUp={onKeyUp}
      onFocus={onFocus}
    >
      <header className="approve-card__who">
        <StatusGlyph kind={isQuestion ? "needs-question" : "needs-permission"} />
        <span className="approve-card__name" dir="auto">
          {session.name}
        </span>
        <span className="approve-card__wait">{formatDuration(request.waitingMs)}</span>
        <button type="button" className="approve-card__menu" aria-label="More actions">
          <svg width="16" height="16" viewBox="0 0 16 16" aria-hidden="true">
            <circle cx="3.5" cy="8" r="1.25" fill="currentColor" />
            <circle cx="8" cy="8" r="1.25" fill="currentColor" />
            <circle cx="12.5" cy="8" r="1.25" fill="currentColor" />
          </svg>
        </button>
      </header>

      <h3 className="approve-card__what" dir="auto">
        {headline(detail)}
      </h3>

      {isQuestion ? (
        <p className="approve-card__question" dir="auto">
          {detail.question}
        </p>
      ) : detail.kind === "edit" ? (
        <pre className="approve-card__payload" dir="auto" data-collapsed={collapsed || undefined}>
          {detail.hunk.map((l, i) => (
            <code key={i} className="approve-card__line" data-kind={l.kind}>
              {l.kind === "add" ? "+ " : l.kind === "del" ? "- " : "  "}
              {l.text}
              {"\n"}
            </code>
          ))}
        </pre>
      ) : (
        <pre className="approve-card__payload" dir="auto" data-collapsed={collapsed || undefined}>
          {lines.join("\n")}
        </pre>
      )}

      {collapsed && (
        <button type="button" className="approve-card__expand" onClick={() => fire("expand")} aria-keyshortcuts="Space">
          {showAllLabel(lines.length)}
        </button>
      )}
      {cwd && (
        <p className="approve-card__cwd" dir="auto">
          in {cwd}
        </p>
      )}
      {risky && risk.reason && (
        <p className="approve-card__risk" id={riskId}>
          <AuxGlyph kind="caution" label="Risky" />
          <span>{risk.reason}</span>
        </p>
      )}

      {isQuestion ? (
        <div className="approve-card__actions approve-card__actions--options" role="group" aria-label="Answers">
          {detail.options.map((o, i) => (
            <button key={o} type="button" className="approve-card__btn" data-kind="option" aria-disabled={disabled} onClick={() => fire(`answer:${i}`)}>
              {o}
            </button>
          ))}
        </div>
      ) : (
        <div className="approve-card__actions">
          <button type="button" className="approve-card__btn" data-kind="deny" aria-disabled={disabled} aria-keyshortcuts="N" onClick={() => fire("deny")}>
            Deny <Kbd k="N" />
          </button>
          {risky ? (
            <button
              type="button"
              className="approve-card__btn"
              data-kind="allow"
              data-holding={holdProgress > 0 || undefined}
              aria-disabled={disabled}
              aria-keyshortcuts="Y"
              aria-describedby={riskId}
              onPointerDown={() => fire(collapsed ? "expand" : "hold-allow")}
              onPointerUp={onHoldEnd}
              onPointerLeave={onHoldEnd}
            >
              <Ring progress={holdProgress} />
              Hold to allow <Kbd k="Y" />
            </button>
          ) : (
            <button type="button" className="approve-card__btn" data-kind="allow" aria-disabled={disabled} aria-keyshortcuts="Y" onClick={() => fire(collapsed ? "expand" : "allow")}>
              Allow once <Kbd k="Y" />
            </button>
          )}
        </div>
      )}

      {!risky && !isQuestion && request.alwaysRule && (
        <button type="button" className="approve-card__link" aria-disabled={disabled} aria-keyshortcuts="A" onClick={() => fire(collapsed ? "expand" : "always")}>
          <span>
            Always allow <code>{request.alwaysRule}</code> in this project
          </span>
          <Kbd k="A" />
        </button>
      )}
      <button type="button" className="approve-card__link" aria-disabled={disabled} aria-keyshortcuts="T" onClick={() => fire("terminal")}>
        <span>Answer in terminal</span>
        <Kbd k="T" />
      </button>
    </div>
  );
}
