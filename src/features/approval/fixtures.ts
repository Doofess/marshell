import type { ApprovalRequest } from "./types";

const MIN = 60_000;
const longCommand = Array.from({ length: 40 }, (_, i) =>
  i === 0 ? "set -euo pipefail" : i % 8 === 0 ? `echo "step ${i}: done"` : `docker build --target stage-${i} -t registry.example.dev/my-app:stage-${i} .`,
).join("\n");

/** One card per state in docs/PLAN.md deliverable 4. Session names match the batch 1 row fixtures where they overlap. */
export const APPROVALS = {
  safeBash: {
    id: "safe-bash",
    session: { name: "billing", agent: "claude" },
    sessionCwd: "C:/dev/payments",
    waitingMs: 65_000,
    detail: { kind: "bash", command: "npm test" },
    risk: { level: "normal" },
    alwaysRule: "Bash(npm test:*)",
    state: { phase: "pending" },
  },
  riskyBash: {
    id: "risky-bash",
    session: { name: "refactor", agent: "claude" },
    sessionCwd: "C:/dev/my-app",
    waitingMs: 12_000,
    detail: { kind: "bash", command: "rm -rf ../old-build", cwd: "C:/dev/my-app/packages/web" },
    risk: { level: "risky", reason: "Deletes files recursively outside this project" },
    state: { phase: "pending" },
  },
  edit: {
    id: "edit",
    session: { name: "api-server", agent: "claude" },
    sessionCwd: "C:/dev/my-app",
    waitingMs: 31_000,
    detail: {
      kind: "edit",
      path: "src/auth.ts",
      add: 12,
      del: 3,
      hunk: [
        { kind: "context", text: "export async function refresh(client: Client) {" },
        { kind: "del", text: "  const token = await client.token();" },
        { kind: "add", text: "  const token = await client.token({ force: true });" },
        { kind: "add", text: "  if (!token) throw new AuthError(\"refresh failed\");" },
        { kind: "context", text: "  return token;" },
        { kind: "context", text: "}" },
      ],
    },
    risk: { level: "normal" },
    alwaysRule: "Edit(src/**)",
    state: { phase: "pending" },
  },
  writeNew: {
    id: "write-new",
    session: { name: "landing", agent: "codex" },
    sessionCwd: "C:/dev/marketing",
    waitingMs: 8_000,
    detail: { kind: "write", path: ".env.example", preview: "API_URL=http://localhost:3000\nSTRIPE_KEY=\nSENTRY_DSN=", isNew: true },
    risk: { level: "normal" },
    state: { phase: "pending" },
  },
  mcp: {
    id: "mcp",
    session: { name: "infra", agent: "claude" },
    sessionCwd: "C:/dev/terraform",
    waitingMs: 95_000,
    detail: { kind: "mcp", server: "github", tool: "create_pr", args: { title: "Upgrade aws provider to 6.x", base: "main", head: "chore/upgrade-aws-provider", draft: "true" } },
    risk: { level: "normal" },
    alwaysRule: "mcp__github__create_pr",
    state: { phase: "pending" },
  },
  longCommand: {
    id: "long-command",
    session: { name: "ci-fix", agent: "gemini" },
    sessionCwd: "C:/dev/my-app",
    waitingMs: 2 * MIN,
    detail: { kind: "bash", command: longCommand },
    risk: { level: "normal" },
    state: { phase: "pending" },
  },
  question: {
    id: "question",
    session: { name: "infra", agent: "gemini" },
    sessionCwd: "C:/dev/terraform",
    waitingMs: 4 * MIN,
    detail: { kind: "question", question: "Which AWS region should the new provider config target?", options: ["us-east-1", "eu-west-1", "ap-southeast-2"] },
    risk: { level: "normal" },
    state: { phase: "pending" },
  },
  receipt: {
    id: "receipt",
    session: { name: "billing", agent: "claude" },
    sessionCwd: "C:/dev/payments",
    waitingMs: 70_000,
    detail: { kind: "bash", command: "npm test" },
    risk: { level: "normal" },
    state: { phase: "decided", verdict: "allowed" },
  },
  answeredInTerminal: {
    id: "answered-in-terminal",
    session: { name: "billing", agent: "claude" },
    sessionCwd: "C:/dev/payments",
    waitingMs: 40_000,
    detail: { kind: "bash", command: "npm test" },
    risk: { level: "normal" },
    state: { phase: "terminal" },
  },
  released: {
    id: "released",
    session: { name: "nightly", agent: "claude" },
    sessionCwd: "C:/dev/perf",
    waitingMs: 120_000,
    detail: { kind: "bash", command: "cargo bench" },
    risk: { level: "normal" },
    state: { phase: "released", afterMs: 120_000 },
  },
} satisfies Record<string, ApprovalRequest>;

export const APPROVAL_LIST: ApprovalRequest[] = Object.values(APPROVALS);
