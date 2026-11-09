import { test } from 'node:test';
import assert from 'node:assert/strict';
import { mkdtempSync, mkdirSync, writeFileSync, readFileSync, rmSync, realpathSync } from 'node:fs';
import { spawnSync } from 'node:child_process';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { editionFor, inside } from './format-rust.mjs';

const hook = path.join(path.dirname(fileURLToPath(import.meta.url)), 'format-rust.mjs');
const hasRustfmt = spawnSync('rustfmt', ['--version'], { encoding: 'utf8' }).status === 0;
const UGLY = 'fn main(){let x=1;println!("{}",x);}\n';
const PRETTY = 'fn main() {\n    let x = 1;\n    println!("{}", x);\n}\n';

/** A workspace whose crate takes its edition from the workspace, as Marshell's crates do. */
function makeWorkspace() {
  const dir = realpathSync.native(mkdtempSync(path.join(tmpdir(), 'fmt-hook-')));
  writeFileSync(path.join(dir, 'Cargo.toml'), '[workspace]\nmembers = ["app"]\n\n[workspace.package]\nedition = "2024"\n');
  mkdirSync(path.join(dir, 'app', 'src'), { recursive: true });
  writeFileSync(path.join(dir, 'app', 'Cargo.toml'), '[package]\nname = "app"\nversion = "0.1.0"\nedition.workspace = true\n');
  return dir;
}

function run(projectDir, input) {
  const r = spawnSync(process.execPath, [hook], {
    input: JSON.stringify({ hook_event_name: 'PostToolUse', cwd: projectDir, ...input }),
    env: { ...process.env, CLAUDE_PROJECT_DIR: projectDir },
    encoding: 'utf8',
  });
  assert.equal(r.status, 0, r.stderr);
  return r.stdout;
}

test('a crate that inherits its edition gets the workspace one', (t) => {
  const dir = makeWorkspace();
  t.after(() => rmSync(dir, { recursive: true, force: true }));
  assert.equal(editionFor(path.join(dir, 'app', 'src', 'main.rs')), '2024');
});

test('no manifest above a file names no edition', (t) => {
  const dir = realpathSync.native(mkdtempSync(path.join(tmpdir(), 'fmt-hook-')));
  t.after(() => rmSync(dir, { recursive: true, force: true }));
  assert.equal(editionFor(path.join(dir, 'loose.rs')), null);
});

test('inside means under the folder, never the folder itself or a sibling', () => {
  const root = path.resolve('/work/repo');
  assert.equal(inside(root, path.join(root, 'src', 'a.rs')), true);
  assert.equal(inside(root, root), false);
  assert.equal(inside(root, path.resolve('/work/repo-other/a.rs')), false);
  assert.equal(inside(root, path.resolve('/work/a.rs')), false);
});

test('an edited Rust file is formatted and Claude is told to read it again', { skip: !hasRustfmt && 'no rustfmt' }, (t) => {
  const dir = makeWorkspace();
  t.after(() => rmSync(dir, { recursive: true, force: true }));
  const file = path.join(dir, 'app', 'src', 'main.rs');
  writeFileSync(file, UGLY);
  const out = JSON.parse(run(dir, { tool_name: 'Edit', tool_input: { file_path: file } }));
  assert.equal(readFileSync(file, 'utf8').replace(/\r\n/g, '\n'), PRETTY);
  assert.equal(out.hookSpecificOutput.hookEventName, 'PostToolUse');
  assert.match(out.hookSpecificOutput.additionalContext, /rustfmt reformatted .*main\.rs/);
});

test('a file already formatted says nothing', { skip: !hasRustfmt && 'no rustfmt' }, (t) => {
  const dir = makeWorkspace();
  t.after(() => rmSync(dir, { recursive: true, force: true }));
  const file = path.join(dir, 'app', 'src', 'main.rs');
  writeFileSync(file, PRETTY);
  assert.equal(run(dir, { tool_name: 'Write', tool_input: { file_path: file } }), '');
});

test('a file rustfmt cannot parse is left as it is', { skip: !hasRustfmt && 'no rustfmt' }, (t) => {
  const dir = makeWorkspace();
  t.after(() => rmSync(dir, { recursive: true, force: true }));
  const file = path.join(dir, 'app', 'src', 'main.rs');
  const broken = 'fn main( {let x=1;\n';
  writeFileSync(file, broken);
  assert.equal(run(dir, { tool_name: 'Edit', tool_input: { file_path: file } }), '');
  assert.equal(readFileSync(file, 'utf8'), broken);
});

test('other files, and Rust files outside the project, are not touched', { skip: !hasRustfmt && 'no rustfmt' }, (t) => {
  const dir = makeWorkspace();
  const other = makeWorkspace();
  t.after(() => {
    rmSync(dir, { recursive: true, force: true });
    rmSync(other, { recursive: true, force: true });
  });
  const ts = path.join(dir, 'app', 'src', 'a.ts');
  writeFileSync(ts, UGLY);
  const outside = path.join(other, 'app', 'src', 'main.rs');
  writeFileSync(outside, UGLY);
  assert.equal(run(dir, { tool_name: 'Write', tool_input: { file_path: ts } }), '');
  assert.equal(run(dir, { tool_name: 'Edit', tool_input: { file_path: outside } }), '');
  assert.equal(readFileSync(ts, 'utf8'), UGLY);
  assert.equal(readFileSync(outside, 'utf8'), UGLY);
});

test('bad input exits quietly', () => {
  const r = spawnSync(process.execPath, [hook], { input: 'not json', encoding: 'utf8' });
  assert.equal(r.status, 0);
  assert.equal(r.stdout, '');
});
