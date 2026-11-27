# Remove the accent picker live check (slice 6z)

- Date: 2026-11-27, 22:59 to 23:05 local time.
- Build: `phase-6z` at `f288573d` under `pnpm tauri dev` on Windows 11 Pro (10.0.26300).
- Home: a scratch `MARSHELL_HOME`, `CLAUDE_CONFIG_DIR` and `MARSHELL_CLAUDE_SETTINGS`, with CDP on port 9333. Onboarding was skipped by the seed's `"onboarded":true`, so nothing saved the settings before the check did.
- Data: `live-check.ps1 start` on a new empty scratch folder, then `stop`. The seed `{"version":1,"onboarded":true,"appearance":{"theme":"light","accent":"teal"}}` was then written to `mh\settings.json` (SHA-256 `94a0544f…29ad1`), and the app started again with `live-check.ps1 restart` on the same folder.
- How it was driven: CDP evaluations, CDP keys (Ctrl+Shift+K, typing in the palette, Enter, Tab, Shift+Tab, ArrowLeft), emulated forced colors (not a real high-contrast theme), one screenshot. No desktop input. No agent session was started.

To read the drawn accent, each check appended a `span` styled `color: var(--accent)`, read its computed `color`, and removed it. WebView2 reports the colour in oklch.

## Results

| Check | Result |
| --- | --- |
| The old accent is read past | Pass. The root carries `lang`, `data-host=desktop` and `data-theme=light`, with no `data-accent` and no inline style. The `--accent` token reads `light-dark(oklch(42% 0.028 70), oklch(80% 0.045 75))`, and the probe draws `oklch(0.42 0.028 70)`, sand in light. `settings.json` still hashes to the seed's SHA-256, so the start did not rewrite it. |
| Appearance shows Theme, High contrast and Text size, and no accent | Pass. Opened from the palette's "Settings: Appearance…". The section's legends are exactly "Theme", "High contrast" and "Text size". The section's text has no "accent", and the page has no swatch element. |
| The keyboard path | Pass. From the "Appearance" heading, Tab goes to Theme (Light, checked), High contrast (Follow system), Text size (Default), then out of the section to "Back to sessions". Shift+Tab goes back the same way. ArrowLeft on Light picks Dark. |
| The next save drops the key | Pass. After Dark was picked, the root has `data-theme=dark` and the probe draws `oklch(0.8 0.045 75)`, sand in dark. `settings.json` has `"theme":"dark"` and no `accent` key; its `appearance` keys are `theme`, `contrast`, `text_size`, `terminal_theme`, `terminal_font`, `contrast_protection` and `imported_themes`. `settings.json.bak` is the seed byte for byte, `"accent":"teal"` included, which is the accepted limit. |
| Sessions keep their colour; the window's own controls draw sand | Pass for the window. A probe inside the shell panel (`.shell-panel`, "PowerShell") and one in its header both draw `oklch(0.8 0.045 75)`. No element on the page has `data-agent`. No session was open, so a session row's vendor colour was left to `accentFollowsVendor.test.ts`, as the plan says. |
| Forced colors (emulated) | Pass. In the screenshot of the Appearance section, every chip's text and outline are readable. The selected chips (High contrast's Follow system, Text size's Default) carry the system highlight border, and Dark, focused, shows the focus ring. Text size's chips end the section, with no space left where the swatches were. |
| Nothing outside the scratch homes changed | Pass. The real `~/.claude/settings.json` kept its modified time (2026-11-26 20:29). `stop` reported all 21 recorded processes gone. |

## Findings

No new faults. The old accent stays in `settings.json.bak` until the save after the one that drops it, as the plan accepted.
