# S15: Running git on a checkout without reading that checkout's config (Windows)

Slice 6r-3, run 2026-11-23 while the slice was planned, after the user chose that Changes follows the shell into any checkout with a hardened git ("Slice 6r-3 decisions" 4 and 5). Dev box: Windows 11 Pro (build 26300), git 2.55.0.windows.3. The scripts were `C:\dev\lc-6r3-spike-tools\spike*.sh`, run on a scratch repo in `C:\dev\lc-6r3-spike\`; nothing in the app changed.

**Verdict: go for a sandbox git folder of Marshell's own. No-go for `-c` flags alone.**

## The question

Slice 6t keeps git out of any folder learned from terminal output, because such a folder can hold a `.git/config` that runs a program (6r-2's PR #170). Changes needs git's diff in exactly those folders. Can git be run there so that nothing in the repository's own config is read or run?

## The trap

A scratch repo (`repo`, branch `feat` three commits ahead of `main`, an uncommitted edit, an untracked file, a renamed file, a CRLF file, and a nested repo added as a submodule) got, written straight into its `.git/config`:

- `core.fsmonitor` running a program that appends to a marker file;
- a `filter.evil.clean` and a `diff.evil.textconv`, named for `*.txt` by a `.gitattributes`;
- `diff.external`;
- an `[include]` of a second file setting `core.pager` and an alias;
- and in the submodule's own config another `core.fsmonitor`.

## Findings

1. **Plain `git diff main` runs the repo's programs.** The marker read `fsmonitor sub-fsmonitor external external external external`: the repo's fsmonitor, the submodule's (git starts a nested git to see whether the submodule is dirty), and the external diff once per file.
2. **`-c` flags don't stop the repo's config.** `git -c core.fsmonitor=false diff --no-ext-diff --no-textconv main` still tried to run the repo's clean filter on a changed `.txt` file (git printed `external filter 'sh -c …' failed`; the probe's own command was cut at a `;`, which git's config parser reads as a comment), and `git config --get core.pager` still returned the value from the included file. An include can name a network path, which on Windows reaches out over SMB. A filter is named per repository, so no fixed flag list can switch every one off.
3. **A git folder of Marshell's own reads nothing of the repo's config.** With `GIT_DIR` set to an empty folder holding only `HEAD`, `refs/` and a two-line `config`, and `GIT_WORK_TREE`, `GIT_INDEX_FILE` and `GIT_OBJECT_DIRECTORY` pointing at the repo's work tree, index and objects (plus `GIT_OPTIONAL_LOCKS=0`, `-c core.fsmonitor=false` and `--ignore-submodules=all`), `merge-base`, `diff --name-status -M`, the full patch, `ls-files --cached` and `rev-list --count` all answered correctly. The marker stayed empty; no filter error was printed; `git config --get core.pager` returned nothing.
4. **It keeps what matters for a diff.** Rename detection worked (`R100 moved-src.txt moved-dst.txt`). The CRLF file showed no change, because the system config's `core.autocrlf=true` still applied. The repo's index file was not rewritten (same modified time before and after).
5. **Worktrees, sha256 and a split index work.** A linked worktree diffs with its own index (`.git/worktrees/<name>/index`) and the common objects. A sha256 repo needs `repositoryformatversion = 1` and `extensions.objectformat = sha256` in the sandbox's config, or git refuses its object names. A split index (`core.splitIndex`) was read correctly.
6. **It is cheap.** On this repo (2,132 tracked files) a sandboxed `diff --raw -z -M` took about 36 ms a run over ten runs, of which about 28 ms is git starting (`git --version` ten times: 0.28 s). The full patch against `origin/main` took 39 ms.
7. **What still reaches git.** The system and user config files are still read (the user's own), so a filter defined there, such as Git LFS's, can still run on a file the repo's `.gitattributes` names. Without the repo's config, git no longer knows the repo's own `core.autocrlf`, its includes, its `info/exclude` and `info/attributes`, or a promisor remote.

## What 6r-3 takes from this

- Git for Changes always runs in a sandbox git folder under Marshell's home, one per checkout, whose `config` Marshell writes: the object format, plus an allowlist of settings that change how files compare, read from the system, user and repository files with `git config --file <f> --no-includes`. Git is also given `GIT_CONFIG_NOSYSTEM=1` and `GIT_CONFIG_GLOBAL` pointing at an empty file, so no config file but Marshell's is read and no config program runs, the user's own LFS filter included ("Slice 6r-3 decisions" 5).
- Marshell resolves the refs itself (HEAD, the branch, the remote's default) from the repo's files, so git never needs the repo's `HEAD` or `refs/`.
- Untracked files come from 6t's own walk (`walk_unignored`), not from git, which still walks a folder link on Windows (#167).
- Before git runs, Marshell refuses a repo whose `objects/info/alternates` names a network or device path, and copies the repo's `shallow` file into the sandbox so a shallow clone's history ends where git expects.
- Known limits: a file stored through a filter (Git LFS, git-crypt) shows as changed once its stat changes, because git no longer runs the filter; a repository that stores its refs as reftable isn't read.
