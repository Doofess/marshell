# Accounts redesigned: sign-in, GitHub, sync and reaching other PCs

- Date: 2026-11-28.
- Purpose: the research behind phase 10's 2026-11-28 decisions in `docs/PLAN.md` §9. Those decisions replace the 2026-10-22 design. The older research is in `2026-10-22-accounts-and-cross-device.md`; it still holds where this note doesn't say otherwise.
- How: four Sonnet web-research agents and one fetch agent read vendor docs, pricing pages and advisories. Nothing was built or run.
- Status: research. Every price and plan limit was read on 2026-11-28 and will change. **[vendor]** marks a vendor's own claim that nobody here has tested.

## 1. What the user asked for

- An optional account that people sign in to with a passkey, Google, GitHub, or email and a password.
- It has three jobs:
  - remember the GitHub account;
  - sync settings between the person's PCs;
  - show terminals on their other PCs, across long distances, more easily than through SSH addresses.
- No phone app.
- Handoffs between PCs were parked.

## 2. Sign-in

### Services compared

| Service | Passkeys | Cost at Marshell's size | Notes |
|---|---|---|---|
| Better Auth | Plugin, stability not stated | Free, MIT; Cloudflare's free tier covers the Worker and D1 at this size | Runs on Cloudflare Workers with D1. Has device-authorization and Electron plugins. We send the email ourselves. |
| Clerk | Only on paid plans, from $25 a month | $25 a month or more | Hosted. |
| WorkOS AuthKit | Included | Free up to 1M monthly users; a custom domain is $99 a month | Hosted. Has a CLI auth flow. |
| Auth0 | Included | Free up to 25k monthly users | Hosted. |
| Supabase Auth | Experimental, beta | Free tier | Self-hostable. |

Sources: better-auth.com/docs (passkey, device-authorization, Electron, Cloudflare, database and email-password pages); github.com/better-auth/better-auth; clerk.com/pricing and its Electron passkeys page; workos.com/pricing and its AuthKit passkeys and CLI-auth pages; auth0.com/pricing and its passkey API page; supabase.com/pricing, its passkeys guide and the passkeys beta changelog.

The user chose **Better Auth on Cloudflare** at **`marshell.bookandry.com`**. bookandry.com is a domain they already use for another project.

### How a desktop app signs in

- **Passkeys belong to a web domain** (the relying-party ID). Marshell's passkeys are therefore made and used on `marshell.bookandry.com`, in the system browser, and never in the webview. Microsoft's MSAL guidance also points desktop apps to the system browser rather than an embedded one (learn.microsoft.com/entra/msal/dotnet/advanced/webview2).
- **Getting the result back:** a loopback redirect to `http://127.0.0.1:<port>` with PKCE, as in RFC 8252 (OAuth 2.0 for native apps).
  - `tauri-plugin-oauth` runs such a loopback listener (docs.rs/crate/tauri-plugin-oauth).
  - A custom-scheme deep link is the other option. Tauri's deep-link plugin then needs the single-instance plugin on Windows (v2.tauri.app/plugin/deep-linking).
  - The loopback avoids a registered URL scheme, so it is the default here.
- **A device code** (Better Auth's device-authorization plugin) is the fallback where no browser can come back to the app.
- **Email and password** need a verification email and a reset email. Better Auth sends neither itself; the app supplies a sender. Which service sends them is left to the spike.
- **The WebAuthn PRF extension** could derive a key from a passkey. **[vendor]** It is in Windows 11 24H2 and 25H2 after KB5077181 (corbado.com/blog/passkeys-prf-webauthn; Bitwarden's community thread on PRF through Windows Hello). Phase 10 doesn't depend on it: the account key comes from approval by a PC (phase 10 decision 4 of 2026-11-28).

## 3. Remembering the GitHub account

- **A GitHub App's user tokens expire by default, and each refresh token works once.** A refresh returns a new pair (docs.github.com, "Refreshing user access tokens"). An app can opt out of expiring user tokens, but by default one login can't be shared by several PCs. A GitHub App must also be installed on each account or organisation whose repositories it touches ("Differences between GitHub Apps and OAuth apps").
- **An OAuth App's token doesn't expire** until it is revoked, or goes unused for a year (docs.github.com, "Token expiration and revocation"). `gh` signs in through an OAuth App, with the scopes `repo`, `read:org` and `gist` by default (cli.github.com/manual/gh_auth_login).
- **The design:**
  - Each PC takes one browser hop to GitHub and keeps its own token in the OS keychain.
  - The account stores the GitHub login and GitHub's numeric user id, so a new PC knows which GitHub account to connect. The PC checks the id after the GitHub sign-in, since a login can be renamed and taken by someone else (PLAN's phase 10 decisions of 2026-11-28).
  - Marshell passes the token to the `gh` it runs through `GH_TOKEN`, and never writes `gh`'s own configuration.
  - No AI tokens are spent.
- Which scopes Marshell's GitHub features need (slices 6r-4a and 6r-4b) is settled when the phase starts.

## 4. Settings sync

- **Synced:** appearance, but for its contrast, which stays on each PC; notifications and status; editor, diff and browser options. PLAN's phase 10 decisions of 2026-11-28 have the final list and the fields still open.
- **Never synced:** folders and paths, the layout, History, shell profiles, own agents, and CLI configurations.
- The candidate fields in today's `settings.json` are `appearance`; `notifications` (its sound switches included) and `status_off`; and `files`, `changes` and `browser`. `close_to_tray` stays per PC. None of them holds a path (`crates/protocol/src/api.rs`); `default_folder` and `history_backup_dir` are separate fields.
- The fields are encrypted end to end under the account key. The newest value wins, setting by setting, with a version for each inner setting (PLAN's phase 10 decisions of 2026-11-28).
- Marshell keeps no secret in the OS keychain yet. The GitHub token (section 3) and the device key would be its first.

## 5. Reaching other PCs

### Why not SSH

- **OpenSSH Server isn't installed on Windows 11 by default.** Installing it and starting `sshd` needs an administrator, and a firewall rule opens port 22 (learn.microsoft.com, "Get started with OpenSSH for Windows").
- **An administrator's keys go in `C:\ProgramData\ssh\administrators_authorized_keys`,** with ACL rules of its own ("OpenSSH key management"). Whether a `command=` restriction on a key holds on Windows was not confirmed (PowerShell/Win32-OpenSSH#1942).
- **SSH still doesn't cross NATs.** Over a long distance it needs port forwarding or a VPN such as Tailscale.
- **Tailscale SSH doesn't run as a server on Windows** (tailscale.com/docs/features/tailscale-ssh). Its Rust library is a preview (tailscale.com/blog/tailscale-rs-rust-tsnet-library-preview).
- **`russh`** is at 0.65.4 and had RustSec advisories in 2026: RUSTSEC-2026-0153 and RUSTSEC-2026-0154.
- **VS Code Remote Tunnels** runs SSH inside a relay that Microsoft hosts, with a GitHub or Microsoft sign-in (code.visualstudio.com/docs/remote/tunnels). It is the closest known pattern, but it is Microsoft's service.

### iroh

- **iroh 1.0 shipped on 2026-06-15** (iroh.computer/blog/v1).
- **Connections:** an endpoint is named by its public key, and the link is QUIC. iroh punches holes through NATs and falls back to a relay.
- **The relay can't read the traffic;** it is encrypted end to end (docs.iroh.computer/concepts/relays).
- **n0's public relays** are free, rate-limited and without uptime guarantees, "suitable for development and testing".
- **n0's paid relays** (iroh.computer/pricing):
  - Pro: $19 a month, shared authenticated relays, a 5 MB/s rate limit and 100 GB of egress, then $0.09/GB.
  - Dedicated: $199 a month more per region.
- **An app chooses its relays with** `Endpoint::builder(...).relay_mode(RelayMode::Custom(RelayMap::from_iter([...])))` (docs.iroh.computer/deployment/dedicated-infrastructure). A relay can admit only listed endpoint ids, or clients holding a token (`RelayMap::with_auth_token`; the iroh-relay README).
- **The design:**
  - The account server introduces the person's PCs to each other, by device key and iroh endpoint id.
  - Each PC pins the others' keys through the account's approvals.
  - Terminal traffic goes PC to PC, or through a relay, and never through the account server.
  - There is no OpenSSH server, no administrator prompt and no port 22.

### An always-on Ubuntu PC at home

The user asked what changes "if i have another desktop with ubuntu which i'll always keep on at home with the server on it". That PC can run **the person's own iroh relay**, and Marshell's Advanced relay setting points at it. Facts from the iroh-relay source and docs (github.com/n0-computer/iroh, `iroh-relay/src/main.rs`; docs.rs `iroh_relay::server::QuicConfig`; docs.iroh.computer/iroh-services/relays/self-hosted):

- **Ports:** HTTP on 80 and HTTPS on 443 (TCP). QUIC address discovery is on UDP 7842, but it is off by default and needs TLS. Metrics are on 9090, which stays inside the house.
- **TLS:** `cert_mode = "LetsEncrypt"` with a `hostname` and a contact email, or manual certificates. It therefore needs a DNS name, such as `relay.bookandry.com`. The ACME HTTP challenge presumably needs port 80 reachable; the docs don't say.
- **Installing:** a release binary or the Docker image `n0computer/iroh-relay`.
- **Reaching it from outside:**
  - Inferred, not documented: the router must forward 80, 443 and (with QUIC address discovery) UDP 7842 to that PC.
  - The home connection needs a public IPv4 address. Behind carrier-grade NAT it needs a tunnel or a forwarded address elsewhere.
  - If the home address changes, it needs dynamic DNS.
- **Limits:** relayed traffic is bounded by the home connection's upload speed and by that PC staying on.

What the Ubuntu PC is not, for now:

- **It is not the account server.** That stays on Cloudflare (phase 10 decision 3 of 2026-11-28).
- **It is not one of "my PCs" with sessions.** Marshell isn't tested on Linux yet, so running Marshell there to host terminals waits for the Linux work.

The relay only carries traffic when a direct link fails, and it never sees the plain text.

## 6. What was not verified

- Nothing was built or run: not iroh's hole punching on Windows, not Better Auth's passkey plugin on Workers, and not the loopback return from the browser.
- Better Auth's passkey plugin doesn't state its stability.
- The WebAuthn PRF claim for Windows is a vendor's.
- Whether a `command=` restriction on an OpenSSH key holds on Windows.
- The firewall and CGNAT needs of a home relay, beyond the ports in iroh-relay's code.
- Which `gh` scopes Marshell's own GitHub features need.
- `GH_TOKEN` is read for github.com only; GitHub Enterprise hosts read `GH_ENTERPRISE_TOKEN`. When `gh` is already signed in as another account, `GH_TOKEN` takes precedence over it, so which account Marshell's `gh` calls use must be settled and shown.
