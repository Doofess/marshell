import { useCallback, useEffect, useRef, useState } from "react";
import type { AccountStatus } from "../../generated/AccountStatus";
import type { Endpoint } from "../../generated/Endpoint";
import { getAccount } from "../../lib/api/accounts";
import type { CompanyAgent } from "../settings/settingsSections";
import { accountLine, CHECKED, CHECKING, NONE_FOUND, UNREACHABLE } from "./accountsText";
import "./AccountsSection.css";

const ORDER: CompanyAgent[] = ["claude", "codex", "antigravity"];
const NAME: Record<CompanyAgent, string> = { claude: "Claude Code", codex: "Codex", antigravity: "Antigravity" };

export type AccountRow = { agent: CompanyAgent; status: AccountStatus | "checking" | "failed" };

function lineOf(row: AccountRow, now: number): string {
  if (row.status === "checking") return CHECKING;
  if (row.status === "failed") return UNREACHABLE;
  return accountLine(row.status, NAME[row.agent], now);
}

/** Settings → Accounts, pure: one card per found CLI, then Check again. */
export function AccountsView({ rows, now, onSignIn, onCheckAgain }: { rows: AccountRow[]; now: number; onSignIn(agent: "claude" | "codex"): void; onCheckAgain(): void }) {
  if (rows.length === 0) return <p className="accounts__empty">{NONE_FOUND}</p>;
  const busy = rows.some((r) => r.status === "checking");
  return (
    <div className="accounts">
      <ul className="accounts__list">
        {rows.map((r) => {
          const needs = typeof r.status === "object" && (r.status.sign_in === "signed_out" || r.status.sign_in === "unknown");
          return (
            <li key={r.agent} className="accounts__row">
              <h3 className="accounts__name">{NAME[r.agent]}</h3>
              <p className="accounts__line">{lineOf(r, now)}</p>
              {needs && (r.agent === "claude" || r.agent === "codex") && (
                <button type="button" className="accounts__btn" onClick={() => onSignIn(r.agent as "claude" | "codex")}>
                  Sign in to {NAME[r.agent]}
                </button>
              )}
            </li>
          );
        })}
      </ul>
      <div>
        <button type="button" className="accounts__btn" disabled={busy} onClick={onCheckAgain}>
          Check again
        </button>
      </div>
    </div>
  );
}

export function AccountsSection({ endpoint, found, onSignIn, refresh, announce }: { endpoint: Endpoint; found: ReadonlySet<CompanyAgent>; onSignIn(agent: "claude" | "codex"): void; refresh: number; announce(text: string): void }) {
  const agents = ORDER.filter((a) => found.has(a));
  const key = agents.join(",");
  const [rows, setRows] = useState<AccountRow[]>(() => agents.map((agent) => ({ agent, status: "checking" })));
  const [now, setNow] = useState(() => Date.now());
  const run = useRef(0);

  const check = useCallback(async () => {
    const id = ++run.current;
    const list = key ? (key.split(",") as CompanyAgent[]) : [];
    setRows(list.map((agent) => ({ agent, status: "checking" })));
    const next = await Promise.all(
      list.map(async (agent): Promise<AccountRow> => {
        try {
          return { agent, status: await getAccount(endpoint, agent) };
        } catch {
          return { agent, status: "failed" };
        }
      }),
    );
    if (id !== run.current) return;
    setNow(Date.now());
    setRows(next);
  }, [endpoint, key]);

  useEffect(() => {
    void check();
    return () => {
      run.current++;
    };
  }, [check, refresh]);

  const again = () => {
    void check().then(() => announce(CHECKED));
  };
  return <AccountsView rows={rows} now={now} onSignIn={onSignIn} onCheckAgain={again} />;
}
