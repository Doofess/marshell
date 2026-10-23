import { test } from 'node:test';
import assert from 'node:assert/strict';
import { mkdtempSync, mkdirSync, writeFileSync, rmSync, realpathSync } from 'node:fs';
import { spawnSync } from 'node:child_process';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const hook = path.join(path.dirname(fileURLToPath(import.meta.url)), 'session-start.mjs');

function git(cwd, ...args) {
  const r = spawnSync('git', ['-c', 'user.name=t', '-c', 'user.email=t@t', '-C', cwd, ...args], {
    encoding: 'utf8',
  });
  assert.equal(r.status, 0, r.stderr);
  return r.stdout;
}

function makeRepo() {
  const dir = realpathSync.native(mkdtempSync(path.join(tmpdir(), 'ss-hook-')));
  git(dir, 'init', '-q', '-b', 'work');
  writeFileSync(path.join(dir, 'a.txt'), 'a\n');
  writeFileSync(path.join(dir, '.gitignore'), '.superpowers/\n');
  git(dir, 'add', '.');
  git(dir, 'commit', '-q', '-m', 'init');
  return dir;
}

function ledger(dir, name, body) {
  const d = path.join(dir, '.superpowers', 'sdd', name);
  mkdirSync(d, { recursive: true });
  writeFileSync(path.join(d, 'progress.md'), body);
}

function run(cwd) {
  const r = spawnSync(process.execPath, [hook], {
    input: JSON.stringify({ cwd, hook_event_name: 'SessionStart' }),
    encoding: 'utf8',
  });
  assert.equal(r.status, 0);
  return r.stdout;
}

function context(stdout) {
  const o = JSON.parse(stdout);
  assert.equal(o.hookSpecificOutput.hookEventName, 'SessionStart');
  return o.hookSpecificOutput.additionalContext;
}

test('prints_branch_and_ledger_tail', () => {
  const dir = makeRepo();
  try {
    writeFileSync(path.join(dir, 'b.txt'), 'b\n');
    ledger(
      dir,
      'plan-x',
      '# SDD ledger — plan: docs/superpowers/plans/2026-10-23-plan-x.md\nTask 1: complete (abc)\nnotes\nTask 2: complete (def123)\nruling: something\n',
    );
    const ctx = context(run(dir));
    assert.match(ctx, /^Branch work, 1 uncommitted files\./);
    assert.match(ctx, /Ledger \.superpowers\/sdd\/plan-x\/progress\.md: plan 2026-10-23-plan-x\.md;/);
    assert.match(ctx, /last: Task 2: complete \(def123\)/);
    assert.match(ctx, /next: Task 3\./);
    assert.doesNotMatch(ctx, /ruling: something/);
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
});

test('next_is_the_first_task_without_a_complete_line', () => {
  const dir = makeRepo();
  try {
    const head = '# SDD ledger — plan: docs/superpowers/plans/p.md\n';
    ledger(dir, 'gap', `${head}Task 1: complete (a)\nTask 3: complete (c)\nTask 4: complete (d)\n`);
    ledger(dir, 'shuffled', `${head}Task 1: complete (a)\nTask 3: complete (c)\nTask 4: complete (d)\nTask 2: complete (b)\n`);
    const ctx = context(run(dir));
    assert.match(ctx, /sdd\/gap\/progress\.md: plan p\.md; done: Tasks 1, 3, 4; last: Task 4: complete \(d\); next: Task 2\./);
    assert.match(ctx, /sdd\/shuffled\/progress\.md: plan p\.md; done: Tasks 1, 2, 3, 4; last: Task 2: complete \(b\); next: Task 5\./);
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
});

test('silent_without_ledger_beyond_branch', () => {
  const dir = makeRepo();
  try {
    const ctx = context(run(dir));
    assert.equal(ctx.split('\n').length, 1);
    assert.equal(ctx, 'Branch work, 0 uncommitted files.');
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
});

test('worktree_shows_only_its_own_ledgers', () => {
  const dir = makeRepo();
  try {
    const wt = path.join(dir, '.claude', 'worktrees', 'wt');
    git(dir, 'worktree', 'add', '-q', '-b', 'wt', wt);
    ledger(dir, 'main-plan', '# SDD ledger — plan: plans/main-plan.md\n');
    const other = path.join(dir, '.claude', 'worktrees', 'other');
    git(dir, 'worktree', 'add', '-q', '-b', 'other', other);
    ledger(wt, 'wt-plan', '# SDD ledger — plan: plans/wt-plan.md\n');
    ledger(other, 'other-plan', '# SDD ledger — plan: plans/other-plan.md\n');
    const fromMain = context(run(dir));
    assert.match(fromMain, /plan main-plan\.md/);
    assert.match(fromMain, /plan wt-plan\.md/);
    assert.match(fromMain, /plan other-plan\.md/);
    const fromWt = context(run(wt));
    assert.match(fromWt, /^Branch wt,/);
    assert.match(fromWt, /plan wt-plan\.md/);
    assert.doesNotMatch(fromWt, /main-plan|other-plan/);
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
});

test('caps_at_15_lines', () => {
  const dir = makeRepo();
  try {
    for (let i = 0; i < 20; i++) {
      ledger(dir, `p${String(i).padStart(2, '0')}`, `# SDD ledger — plan: plans/p${i}.md\nTask 1: complete (x)\n`);
    }
    const ctx = context(run(dir));
    assert.equal(ctx.split('\n').length, 15);
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
});

test('fails_open_outside_git', () => {
  const dir = realpathSync.native(mkdtempSync(path.join(tmpdir(), 'ss-hook-nogit-')));
  try {
    assert.equal(run(dir), '');
    const bad = spawnSync(process.execPath, [hook], { input: 'not json', encoding: 'utf8' });
    assert.equal(bad.status, 0);
    assert.equal(bad.stdout, '');
    const empty = spawnSync(process.execPath, [hook], { input: '', encoding: 'utf8' });
    assert.equal(empty.status, 0);
    assert.equal(empty.stdout, '');
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
});
