# S7: FTS5 via rusqlite bundled

Pass criterion: an FTS5 table is created and matched (with snippet()) on Windows, macOS and Linux.

| OS | Result | Evidence |
|---|---|---|
| Windows 11 (dev box) | PASS | `cargo test -p marshell-core --test fts5`, 2026-10-09 |
| macOS (CI) | | CI run link |
| Linux (CI) | | CI run link |
