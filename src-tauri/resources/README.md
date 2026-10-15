# Bundled resources

`conpty/` is not in git. On Windows, `pnpm fetch:conpty` downloads it (see `scripts/fetch-conpty.mjs`). `build.rs` watches this folder, so a fetch triggers a rebuild.
