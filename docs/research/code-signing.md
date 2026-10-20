# Code signing checklist

- Date: 2026-10-20

Purpose: PLAN §9 slice 3a decision 6 asked for eligibility, cost and the steps to apply for code signing. Buying certificates and passing identity checks are the user's own tasks. Wiring signing into CI waits for the release phase. Every fact below carries a source and "checked 2026-10-20"; anything not confirmed is marked **unconfirmed**.

## 1. Windows: Azure Artifact Signing (formerly Trusted Signing)

- **Name and status.** Renamed from Trusted Signing; Microsoft calls it a name change only, with functionality, billing, quotas and APIs unchanged. It is a live paid service with Basic and Premium SKUs, not a preview. Sources: https://azure.microsoft.com/en-us/products/artifact-signing, https://learn.microsoft.com/en-us/azure/artifact-signing/overview (checked 2026-10-20).
- **Eligibility (the key answer).** Public Trust certificates are open to organisations in the US, Canada, EU, UK, Australia, New Zealand, Japan, South Korea, Singapore, Switzerland, Norway and **Israel**. Individual developers "must be located in the United States or Canada". Source: https://learn.microsoft.com/azure/artifact-signing/quickstart (page updated 2026-10-08, checked 2026-10-20).
  - **An individual in Israel cannot use it today** for publicly trusted certificates.
  - **An Israeli registered business can** (organisation route). Private Trust has no geographic limit, but it does not help public downloads.
  - Unconfirmed: whether an Israeli sole proprietor (osek murshe/patur) counts as an "organization or DBA". The quickstart offers an "Organization or a DBA" form, but names no Israeli entity types. A 2026 Q&A thread reports a Polish sole proprietor failing validation (anecdotal): https://learn.microsoft.com/en-us/answers/a/12812520.
  - Unconfirmed: a "3+ years of verifiable business history" rule existed in 2025 (https://ayakael.net/mirrors/electron/commit/d1c2d06baebefda14fc1253b47a199435cb8baeb). The current quickstart does not mention it. Ask Microsoft before paying for a company.
- **Price.** Basic $9.99 a month (5,000 signatures, 1 certificate profile per type). Premium $99.99 a month (100,000 signatures, 10 profiles). Overage $0.005 per signature. Not pro-rated. Sources: https://azure.microsoft.com/en-us/products/artifact-signing, https://learn.microsoft.com/en-us/azure/artifact-signing/faq (checked 2026-10-20). The pricing page itself shows "$-" in the cached fetch.
- **What you need.** A Microsoft Entra tenant, a **paid** Azure subscription (free, trial and sponsored are refused), the `Microsoft.CodeSigning` provider registered, an account in a supported region (for example West Europe or North Europe), the Identity Verifier role, then an identity validation and a certificate profile. Source: quickstart and FAQ above.
- **Organisation validation.** Needs the legal entity name, a website, a monitored email on the entity's domain, a business identifier and the registered address. The email verification link expires after 7 days. Microsoft states **1 to 20 business days**, longer if documents are requested; documents must be under 12 months old; three upload attempts, then the request is closed. Public records must match exactly. Source: quickstart and FAQ.
- **CI.** Official GitHub Action `azure/artifact-signing-action@v2` (Windows runners only, OIDC login via `azure/login`, role "Artifact Signing Certificate Profile Signer"). Source: https://github.com/Azure/artifact-signing-action. Tauri: `bundle > windows > signCommand` with `cargo install artifact-signing-cli`, for example `artifact-signing-cli -e <endpoint> -a <account> -c <profile> -d Marshell %1`, with `AZURE_CLIENT_ID`, `AZURE_TENANT_ID`, `AZURE_CLIENT_SECRET`. Source: https://v2.tauri.app/distribute/sign/windows/ (the old name was `trusted-signing-cli`).
- **Certificate lifetime and SmartScreen.** Certificates are valid **three days** and rotate automatically, so timestamping (`http://timestamp.acs.microsoft.com`) is mandatory. Source: https://learn.microsoft.com/en-us/azure/artifact-signing/how-to-signing-integrations. Reputation builds from download volume and clean behaviour. It can take weeks and hundreds of clean installs, and a consistent signing identity carries reputation across releases. Source: https://learn.microsoft.com/en-us/windows/apps/package-and-deploy/smartscreen-reputation. Artifact Signing issues no EV certificates and has no plan to.

## 2. Windows alternatives

- **OV certificate.** Since 1 June 2023 the private key must live in certified hardware (token, HSM or cloud HSM), so a downloadable `.pfx` is no longer sold. Source (vendor summaries, CA/B Forum text not read): https://venafi.com/blog/new-ca-b-forum-requirements-for-code-signing-start-in-june-will-you-be-ready. Tauri's `.pfx` guide applies only to certificates bought before 1 June 2023 (https://v2.tauri.app/distribute/sign/windows/).
- **EV no longer bypasses SmartScreen.** "EV certificates no longer bypass SmartScreen... Paying a premium for EV solely to avoid SmartScreen warnings is no longer justified." Source: SmartScreen page above (updated 2026-08-17).
- **Certum Open Source Code Signing.** EUR 69 for a kit with a smart card and reader. Issued only to individuals, with the subject "Open Source Developer". The project must be public and show your relationship to it. Certum can revoke it if you sign commercially distributed software. Needs ID verification plus a utility bill, and no country restriction is listed. Sources: https://shop.certum.eu/open-source-code-signing.html, https://support.certum.eu/en/code-signing-required-documents/. Catch: a physical card cannot sit on GitHub-hosted runners, so release signing would be manual or on a self-hosted runner. Unconfirmed: certificate validity term (Certum says validity is shrinking from March 2026) and whether a cloud variant exists.
- **SignPath Foundation (free for open source).** Conditions: OSI licence with no dual licensing, no proprietary parts, actively maintained, already released, own binaries only, no hacking tools, MFA for all team members, defined author/reviewer/approver roles, manual approval for each release. The project must publish a "Code signing policy" page (team roles, privacy statement, SignPath attribution). The certificate is issued to **SignPath Foundation**, which is the publisher shown. The Foundation decides on acceptance, with no appeal. Sources: https://signpath.org/terms, https://signpath.org/. GitHub: `actions/upload-artifact` then `signpath/github-action-submit-signing-request@v3`; the secret is `SIGNPATH_API_TOKEN`; OSS projects need GitHub-hosted runners. Source: https://docs.signpath.io/trusted-build-systems/github. Unconfirmed: review time, and whether a young or single-maintainer project is accepted.

## 3. macOS

- **Cost.** Apple Developer Program is 99 USD a year (local currency shown at enrolment). Source: https://developer.apple.com/programs/enroll/.
- **Individual vs organisation.** Individual: 2FA Apple Account and legal age; your personal name is the seller. Organisation: a legal entity (DBAs and trade names refused), a **D-U-N-S number**, legal authority, a work email on the organisation's domain and a real public website. Source: https://developer.apple.com/support/enrollment/. Unconfirmed: Israel enrolment availability (neither page lists countries); check at enrolment.
- **Certificate.** "Developer ID Application" for distribution outside the App Store. Only the Account Holder can create it, and notarisation is required. Source: https://v2.tauri.app/distribute/sign/macos/.
- **Notarisation.** Done by Tauri with `notarytool` when the env vars below are set; Tauri also staples (it has a `--skip-stapling` flag). The Apple page itself could not be fetched (unconfirmed detail).
- **Tauri inputs.** `APPLE_SIGNING_IDENTITY` or `bundle > macOS > signingIdentity`; `APPLE_CERTIFICATE` (base64 `.p12`) and `APPLE_CERTIFICATE_PASSWORD`. Notarisation by Apple ID: `APPLE_ID`, `APPLE_PASSWORD` (app-specific), `APPLE_TEAM_ID`. Or by API key: `APPLE_API_ISSUER`, `APPLE_API_KEY`, `APPLE_API_KEY_PATH`. Signing needs an Apple machine (`macos-latest` in CI). Source: the Tauri macOS page above.

## 4. Linux

- Signing is optional: "artifact signing is not required for your application to be deployed on Linux". Tauri signs AppImages with GPG through `SIGN=1`, `SIGN_KEY`, `APPIMAGETOOL_SIGN_PASSPHRASE`; users must verify the signature themselves. Source: https://v2.tauri.app/distribute/sign/linux/ (checked 2026-10-20).
- Unconfirmed (not researched this pass): signed apt/rpm repositories and what Flathub signs. Flathub's submission page describes a pull-request review and says nothing about developer signing: https://docs.flathub.org/docs/for-app-authors/submission.

## 5. Recommended order for Marshell

**Windows route, first try: SignPath Foundation.** Reasoning: it is free, it needs no personal ID or company, it works from an individual's MIT repo, and it fits GitHub-hosted CI. The cost is that the publisher shows as the Foundation, and the Foundation must accept the project (apply after the first public release). Second: Certum Open Source (EUR 69, individual) if SignPath declines, accepting manual signing. Third: Azure Artifact Signing, only if the user registers an Israeli business and Microsoft confirms it qualifies; an individual in Israel is not eligible.

**The user does themselves:**
- Apply to SignPath Foundation, after the first public release exists.
- Or buy the Certum kit and complete its ID check.
- Or, for Azure: a registered business, a paid Azure subscription and the identity validation (1 to 20 business days).
- Apple Developer Program (99 USD a year), then create the Developer ID Application certificate and an app-specific password or API key.
- Publish the Code signing policy page for SignPath.

**The release phase wires into CI later (secrets live only in GitHub repository secrets, never in the repo):**
- Windows: `SIGNPATH_API_TOKEN`; or `AZURE_CLIENT_ID`, `AZURE_TENANT_ID`, `AZURE_SUBSCRIPTION_ID` (OIDC) or `AZURE_CLIENT_SECRET`, plus `bundle > windows > signCommand`.
- macOS: the `APPLE_*` names in section 3 and `KEYCHAIN_PASSWORD` for the temporary CI keychain.
- Linux: `SIGN_KEY` and `APPIMAGETOOL_SIGN_PASSPHRASE`, only if AppImage signing is wanted.
- Expect SmartScreen warnings on the first releases even when signed; reputation builds with downloads.
