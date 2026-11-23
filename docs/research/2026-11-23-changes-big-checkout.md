# What a Changes check costs on a big checkout

Measured on 2026-11-23 for issue #176, before slice 6u was planned. The decisions that rest on these numbers are in `docs/PLAN.md` §9, "Slice 6u decisions (2026-11-23)".

## What was asked

Issue #176: on a checkout with thousands of tracked folders the link safety check (`worktree::reach`, `crates/core/src/gitbox/worktree.rs`) runs on every check and shares the check's 2 s budget (`CHECK_BUDGET`) with git. It was measured only on this repository (2,193 files, about 9 ms). How slow does it get, and what makes it cheaper?

## Setup

- The development PC: Windows 11 Pro 26300, NTFS on drive C:, git for Windows, Microsoft Defender on.
- Two synthetic checkouts, each one commit and no changes: 20,000 files in 4,000 leaf folders (4,441 folders with a tracked path) and 100,000 files in 20,000 leaf folders (22,201 folders). Five two-line files per leaf folder, three levels deep.
- Timed by the test `timing_on_a_big_checkout` in `crates/core/src/changes/mod.rs`, which runs the real check (`compute`) and its parts against the checkout that `MARSHELL_BENCH_CHECKOUT` names, with a Marshell home in a temp folder.
- All numbers are with the files in memory (warm). A cold disk was not measured: emptying the file cache needs a restart or administrator rights.

To run it again, make a checkout (the folder must be new):

```python
import os, subprocess, sys
root, tops = sys.argv[1], int(sys.argv[2])  # 40 gives 20,000 files, 200 gives 100,000
for a in range(tops):
    for b in range(10):
        for c in range(10):
            d = os.path.join(root, "pkg%03d" % a, "mod%d" % b, "part%d" % c)
            os.makedirs(d)
            for f in range(5):
                with open(os.path.join(d, "file%d.txt" % f), "w", newline="\n") as fh:
                    fh.write("line one of %d/%d/%d/%d\nline two\n" % (a, b, c, f))
git = ["git", "-c", "core.autocrlf=false", "-c", "user.name=bench", "-c", "user.email=bench@example.invalid"]
subprocess.run(git + ["init", "-q", "-b", "main"], cwd=root, check=True)
subprocess.run(git + ["add", "-A"], cwd=root, check=True)
subprocess.run(git + ["commit", "-q", "-m", "synthetic checkout"], cwd=root, check=True)
```

Then, with `MARSHELL_BENCH_CHECKOUT` set to that folder:

```
cargo test --release -p marshell-core -j 4 --lib timing_on_a_big_checkout -- --ignored --nocapture
```

## Results (release build, warm, before any change)

| Part of a check | 20,000 files | 100,000 files |
| --- | --- | --- |
| Copying the index when it didn't change (`refresh`) | 2 ms | 2 ms |
| `git ls-files` and its parse (once per change of the index) | 57 ms | 152 ms |
| Link safety check (`reach`), every check | 165 ms | 1.0 s |
| `git diff --raw` | 105 ms | 325 ms |
| Look for new files (`walk_unignored`), at most every 10 s when over 250 ms | 0.86 s | 2.56 s |
| A whole check when the look for new files is reused | 0.26 s | 1.3 s |

A debug build, which `pnpm tauri dev` runs, is about a third slower on the link check (250 ms and 1.3 s).

So the budget of 2 s is not used up at 20,000 files. At 100,000 the link check alone takes half of it, the look for new files is always cut ("Looking for new files took too long, so some may not be shown."), and anything that slows the disk (a cold cache, a virus scan) tips the check into "Git took too long to read the changes."

## What the link check's time is

One listing per folder with a tracked path: about 40 µs each (0.9 s for 22,201 folders). The rest of the check is small.

Three ways to make it cheaper were measured:

| Way | 100,000 files | Notes |
| --- | --- | --- |
| Ask each folder for its modified time, list only the ones that moved | 0.6 s for the asking alone | Dropped. See below. |
| List the folders on 2, 4, 8 threads | 0.63 s, 0.40 s, 0.29 s | No loosening, but the same work on every poll, on more cores. |
| Reuse a "clear" answer for 10 s when the check was slow | 0 s between looks | Chosen (decision 1). |

### Why a folder's modified time doesn't help

A probe on NTFS (a folder `parent/leaf`, changes made inside `leaf`):

| Change in `leaf` | `leaf`'s time in `parent`'s listing moved at once | `leaf`'s own time moved |
| --- | --- | --- |
| A file created | no | yes |
| A file deleted | no | yes |
| A file renamed | yes | yes |
| A file written to | no | no |
| A file replaced by a junction | yes | yes |
| A folder replaced by a junction | yes | yes |

- The time a parent's listing reports for a folder is NTFS's copy in the parent's index, and it is updated late. It can't be trusted, so each folder would have to be asked itself.
- Asking each folder itself costs about 28 µs, against 40 µs for listing it. That saves a third at best, for a check that then trusts a time any program can set back.
- Two changes inside one tick of the clock leave the time the same: creating and then deleting a file kept the folder's time in 154 of 200 tries. A cache keyed on it would need git's "racy" rule as well.

## The look for new files

It is the slowest part, and it cost about four times a plain listing of the same folders. Per folder it listed the folder and then asked the disk three more times for names the listing had just given: `.gitignore` twice (once for whether it is a link, once to read it) and `.git` once (whether the folder is a nested checkout). Each ask that finds nothing costs about 25 µs.

With the listing answering those (slice 6u decision 4):

| | 20,000 files | 100,000 files |
| --- | --- | --- |
| Before | 0.86 s | 2.56 s |
| After | 0.22 s | 1.17 s |

At 20,000 files the look is now under 250 ms, so it runs on every check and not every 10 s: a check there takes about 0.46 s, where it took 0.26 s plus 0.86 s every 10 s.

## After slice 6u

The same test, release build, warm, with the listing answering the walk (decision 4) and a slow link check standing for 10 s (decision 1). Six checks in a row, nothing changed in the checkout:

| | 20,000 files | 100,000 files |
| --- | --- | --- |
| Check 1 (everything runs) | 0.68 s | 2.67 s |
| Checks 2 to 6 | 0.39 to 0.43 s | 0.26 to 0.28 s |
| Checks 2 to 6 before | 0.26 s, plus 0.86 s every 10 s | 1.3 s |
| Link safety check alone | 135 ms | 0.71 to 0.80 s |
| `git diff --raw` | 74 ms | 301 ms |
| Look for new files alone | 183 ms | 1.02 s |

- At 100,000 files a check that has to do both the link check and the look for new files needs about 2.1 s (0.75 s, 0.3 s for `git diff`, 1.0 s) against a budget of 2 s, so the look for new files is cut in it and the answer comes with "Looking for new files took too long, so some may not be shown." That is the first check. After it each of the two runs again once in 10 s, on its own clock: worked out from the panel's 2 s poll, they fall into different checks from about 24 s on and into the same check about once a minute, where the notice shows again until the next full look. The live check looks at this in the running app.
- At 20,000 files both parts are under 250 ms, so neither is reused and every check does all the work: slower between looks than the bigger checkout, with nothing shown late.
- The tracked paths stayed one set across the six checks (the link check was reused each time), so git's own refresh of the sandbox's index copy doesn't end the reuse.

## `tasks::task_at`

Issue #176's comment: it reads the tasks file on every tree and Changes poll. Measured: 0.07 ms with no tasks file and 3.2 ms with 50 tasks. Left as it is.

## What these numbers don't cover

- A cold disk.
- A repository with long paths, many files per folder or deep nesting. The cost follows the number of folders, not of files: this shape (five files a folder) is a hard one.
- Linux and macOS.
