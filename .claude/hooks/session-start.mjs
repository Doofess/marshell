// SessionStart hook: tells a resuming session which branch it is on and which
// plan ledger is active. Plain Node, no dependencies. Fails open: any error
// exits 0 with no output. Output is capped at 15 lines and never includes more
// than the first and last relevant line of a ledger.
import { readFileSync, readdirSync, existsSync, realpathSync } from 'node:fs';
import { spawnSync } from 'node:child_process';
import path from 'node:path';

const MAX_LINES = 15;
const MAX_LAST = 160;

function git(cwd, args) {
  const r = spawnSync('git', ['-C', cwd, ...args], { encoding: 'utf8', timeout: 1000 });
  if (r.error || r.status !== 0) return null;
  return r.stdout;
}

function findCheckout(start) {
  let dir = start;
  for (;;) {
    if (existsSync(path.join(dir, '.git'))) return dir;
    const up = path.dirname(dir);
    if (up === dir) return null;
    dir = up;
  }
}

function subdirs(dir) {
  try {
    return readdirSync(dir, { withFileTypes: true })
      .filter((e) => e.isDirectory())
      .map((e) => path.join(dir, e.name));
  } catch {
    return [];
  }
}

// Every .superpowers/sdd/*/progress.md directly under a checkout root.
function ledgersIn(root) {
  return subdirs(path.join(root, '.superpowers', 'sdd'))
    .map((d) => path.join(d, 'progress.md'))
    .filter((f) => existsSync(f));
}

function describe(file, base) {
  const text = readFileSync(file, 'utf8');
  const lines = text.split(/\r?\n/);
  const first = (lines[0] ?? '').trim();
  // The first line reads `# SDD ledger — plan: <plan path>`.
  const planPath = /\bplan:\s*(\S+)/i.exec(first)?.[1] ?? '';
  const plan = planPath ? planPath.split(/[\\/]/).pop() : 'unknown';
  let last = null;
  let n = 0;
  for (const line of lines) {
    const m = /Task (\d+): complete/.exec(line);
    if (m) {
      last = line.trim();
      n = Number(m[1]);
    }
  }
  const rel = path.relative(base, file).split(path.sep).join('/');
  const tail = last === null ? 'none' : last.slice(0, MAX_LAST);
  return `Ledger ${rel}: plan ${plan}; last: ${tail}; next: Task ${n + 1}.`;
}

function main() {
  const raw = readFileSync(0, 'utf8');
  const input = JSON.parse(raw);
  if (!input || typeof input.cwd !== 'string' || input.cwd === '') return;
  const cwd = realpathSync.native(input.cwd);
  const root = findCheckout(cwd);
  if (!root) return;

  const branch = (git(cwd, ['rev-parse', '--abbrev-ref', 'HEAD']) ?? '').trim();
  if (!branch) return;
  const status = git(cwd, ['status', '--porcelain']) ?? '';
  const count = status.split(/\r?\n/).filter((l) => l.trim() !== '').length;
  const out = [`Branch ${branch}, ${count} uncommitted files.`];

  // The main checkout is the parent of the common git dir.
  let main = null;
  const common = git(cwd, ['rev-parse', '--git-common-dir']);
  if (common) {
    const dir = realpathSync.native(path.resolve(cwd, common.trim()));
    if (path.basename(dir) === '.git') main = path.dirname(dir);
  }

  const files = [];
  const seen = new Set();
  const add = (list, base) => {
    for (const f of list) {
      const key = realpathSync.native(f);
      if (seen.has(key)) continue;
      seen.add(key);
      files.push({ f, base });
    }
  };
  add(ledgersIn(root), root);
  // Only a session in the main checkout lists the worktrees' ledgers; a worktree sees its own.
  if (main && realpathSync.native(main) === realpathSync.native(root)) {
    for (const wt of subdirs(path.join(main, '.claude', 'worktrees'))) {
      add(ledgersIn(wt), main);
    }
  }
  files.sort((a, b) => a.f.localeCompare(b.f));
  for (const { f, base } of files) {
    if (out.length >= MAX_LINES) break;
    try {
      out.push(describe(f, base));
    } catch {
      // skip an unreadable ledger
    }
  }

  process.stdout.write(
    JSON.stringify({
      hookSpecificOutput: { hookEventName: 'SessionStart', additionalContext: out.join('\n') },
    }),
  );
}

try {
  main();
} catch {
  // fail open: no output
}
process.exit(0);
