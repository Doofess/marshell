# Security policy

## Reporting a vulnerability

Please report security issues privately. On the repository's **Security** tab, choose **Report a vulnerability**, or open [a new advisory](https://github.com/Doofess/marshell/security/advisories/new) directly. Please don't open a public issue for a vulnerability.

A useful report says:

- what you found and what an attacker could do with it;
- the steps to reproduce it;
- the commit or version, and your OS.

Expect an acknowledgement within a week. Once a fix is ready, it is published together with an advisory, and you are credited unless you'd rather not be.

## Supported versions

Marshell is in early development and has no release yet. Security fixes land on `main`. Once releases start, the latest release gets security fixes.

## Security model

This section describes what Marshell does today. Planned work is marked as such.

**Local API.**
- The interface talks to Marshell's Rust core over HTTP and WebSocket on `127.0.0.1`, on a random port.
- Every request must carry a random 256-bit token for that run. HTTP requests send it as a bearer token; WebSocket connections send it as a subprotocol entry.
- The `Host` header must be exactly `127.0.0.1:<port>`, which blocks DNS rebinding.
- The port and the token are written to `~/.marshell/run/endpoint.json`.
  - On Linux and macOS the file is owner-only (0600, in a 0700 folder).
  - On Windows it inherits the permissions of your user profile.
  - An explicit owner-only ACL on Windows is planned for when the hook bridge starts reading the file.

**Webview.**
- The Content Security Policy allows content from the app itself and connections only to the app and loopback. No remote content is loaded.
- HTML files open in a sandboxed frame that has no scripts, no forms, no navigation and no network access.

**Terminal input.**
- Marshell never presses Enter in a terminal for you.
- Features that put text in an agent's prompt, such as sending a marked region of a PDF, paste the text and leave sending it to you.

**Files.**
- The preview reads only regular files you can already read, through the token-checked API, with size limits.
- Image crops are saved under `~/.marshell/attachments/` and are owner-only.

**Your CLI configuration.**
- Marshell does not write to any agent CLI's configuration today.
- Later phases add hook entries and a status line. Every such change will be previewed, backed up, written atomically and undoable, and uninstalling removes it.

**Credentials.**
- Marshell never reads, stores or brokers CLI credentials or OAuth tokens.
- Signing in runs the CLI's own login command in a terminal.

**Processes.**
- Sessions run as your user.
- On Windows, Marshell and the sessions it starts share a Job Object. If Marshell crashes or is killed, no shell or agent is left running.

### Out of scope

- An attacker who already runs code as your user. They can read the token file and drive your CLIs directly.
- Vulnerabilities in the agent CLIs themselves, such as Claude Code or Codex CLI. Report those to their vendors.

### Planned

Before approve-from-anywhere ships (phase 3 of [`docs/PLAN.md`](docs/PLAN.md)), two pieces of work come first:

- a written threat model, covering the loopback server, terminal input, the `marshell://` link and instruction-library injection;
- a security review.
