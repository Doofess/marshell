# Config safety live check (slice 3b)

- Date: 2026-10-18
- Build: `phase-3b` at 01b096f, `pnpm tauri dev` on Windows 11 Pro (26300)
- Home: a scratch `MARSHELL_HOME` with spaces in its path (`%LOCALAPPDATA%\Temp\marshell live 3b\home dir`), and a scratch project folder next to it
- Driven over CDP (`--remote-debugging-port=9333`): the webview fetched `core_endpoint` and called the API itself, so the token never left the page

## Results

| Check | Result |
|---|---|
| Home ACL before launch | Inherited from `Temp`: the user and SYSTEM with full control, Administrators F, and `CodexSandboxUsers` and one other SID with Modify |
| Home ACL after launch (`icacls`) | Pass: only the user and `NT AUTHORITY\SYSTEM`, both `(OI)(CI)(F)`, with no inherited entries |
| `run/endpoint.json`, `snapshots/` | Pass: inherit those two entries only (`(I)(F)` and `(I)(OI)(CI)(F)`) |
| JsonSet into a new `<project>/.claude/settings.local.json` | Pass: the preview showed a 5-line add with `can_apply: true`, and apply returned 200 with a snapshot id. The file on disk matched the diff |
| JsonSet into a JSONC file with `// line` and `/* inline */` comments | Pass: the diff touched only `env`, and both comments were still on disk after apply |
| Outside edit between preview and apply | Pass: `GET /v1/config/preview/{id}` turned `stale: true` 389 ms after the write, polling included. Apply returned 409 `changed outside Marshell` and the file kept the outside edit |
| Restore the first snapshot through `/v1/snapshots/{id}/restore` | Pass: the preview showed the file removed, and apply returned 200 with a new snapshot. The file was gone |
| Preview `<project>/managed-settings.json` | Pass: refused with reason `managed` and `can_apply: false`. Nothing was created |

## Notes

- The first JSONC attempt ran against a file that was never written, because the shell's `Set-Content -NoNewline` failed. The service treated it as a JsonSet on a missing file, which is the documented behaviour, and created `{ "env": … }`. The check was then repeated against a file written with comments, and that is the result in the table.
- `managed.json.bak` sits next to `managed.json` in the home after the applies: `store::write` keeps the previous copy as `.bak`.
- The app and its `pnpm`, `cargo`, `node` and WebView2 children were stopped by the PIDs started for this check, and the scratch folders were removed.
