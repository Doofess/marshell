# S7: FTS5 via rusqlite bundled

Pass criterion: an FTS5 table is created and matched (with snippet()) on Windows, macOS and Linux.

| OS | Result | Evidence |
|---|---|---|
| Windows 11 (dev box) | PASS | `cargo test -p marshell-core --test fts5`, 2026-10-09 |
| macOS (CI) | PASS (`bundled_sqlite_has_fts5 ... ok`) | [run](https://github.com/Doofess/marshell/actions/runs/37999636586) |
| Linux (CI) | PASS (`bundled_sqlite_has_fts5 ... ok`) | [run](https://github.com/Doofess/marshell/actions/runs/37999636586) |
