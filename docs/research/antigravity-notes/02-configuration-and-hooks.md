# Antigravity CLI: Configuration & Hooks Installation

## Configuration Hierarchy
Antigravity discovers customizations by traversing directories from the current working directory (CWD) up to the repository root (looking for `.agents/`, `.agent/`, `_agents/`, `_agent/`), and finally falling back to the global configuration `~/.gemini/config/`.

For the CLI specifically, the global settings are located at:
`~/.gemini/antigravity-cli/settings.json`

## Hooks Installation Plan (`Adapter::hook_install_plan`)
To inject Marshell into the Antigravity CLI's lifecycle, the adapter needs to write a `hooks.json` file.
Marshell should install its hook definition at the user level, e.g., `~/.gemini/config/hooks.json`.

### Required Hook Configuration (`hooks.json`)
The installer should ensure the following structure exists in `hooks.json`:

```json
{
  "marshell-bridge": {
    "enabled": true,
    "PreToolUse": [
      {
        "matcher": "*",
        "hooks": [
          {
            "type": "command",
            "command": "<path-to-marshell-hook>",
            "timeout": 600
          }
        ]
      }
    ],
    "PostToolUse": [
      {
        "matcher": "*",
        "hooks": [
          {
            "type": "command",
            "command": "<path-to-marshell-hook>",
            "timeout": 30
          }
        ]
      }
    ],
    "Stop": [
      {
        "type": "command",
        "command": "<path-to-marshell-hook>",
        "timeout": 30
      }
    ]
  }
}
```

### Safety & UX Notes
- Before applying this change, Marshell MUST show the user the exact diff (via `ChangeSet`) to comply with the project's non-negotiables.
- The `timeout` for `PreToolUse` is set to 600 seconds to allow the user ample time to approve or deny the request via the Marshell GUI.
- You must expand `~` manually if specifying it in the path, though providing the absolute path to `marshell-hook` is safer.

## The settings file, verified 2026-10-27

Sources: https://antigravity.google/docs/settings (`/docs/cli/settings` redirects there), https://antigravity.google/docs/cli/reference, https://antigravity.google/docs/cli/using, https://antigravity.google/docs/models and the JSON Schema at https://antigravity.google/schemas/v1/settings.json. Read on 2026-10-27, against agy 1.3.x. This corrects the "Schema: none" entry and the `enableTelemetry` entry in `docs/research/2026-10-16-accounts-cloud-extensions.md`.

| Key | Type | Values | Default |
| --- | --- | --- | --- |
| `model` | string, free | the model's label as `/model` shows it ("Gemini 3.1 Pro", "Claude Opus 5.5 (Thinking)"); `agy models` lists them | `""` |
| `toolPermission` | string | `request-review`, `proceed-in-sandbox`, `always-proceed`, `strict` | `request-review` |
| `enableTerminalSandbox` | boolean | `true`, `false` | `false` |
| `colorScheme` | string | `terminal`, `light`, `solarized light`, `colorblind-friendly light`, `dark`, `solarized dark`, `colorblind-friendly dark`, `tokyo night` | `terminal` |
| `verbosity` | string | `high`, `medium`, `low` | `medium` |
| `enableTelemetry` | deprecated | "Not read from settings.json." Telemetry in `/config` "is saved with your account, and the CLI removes this key when it saves the file." | none |

How agy treats the file:
- It is "a plain JSON file". The docs never mention comments or trailing commas. An invalid file opens a "Settings Error" screen whose "Overwrite the file with defaults" keeps a copy as `settings.json.corrupted.<timestamp>`.
- "When the CLI starts with a valid file, it rewrites the file in its own format", and saves "only the keys whose values differ from the defaults". It "replaces the file in one step" with mode `0600`. Comments and formatting don't survive (inferred).
- It also writes the file from `/config` (alias `/settings`), `/model`, `/effort`, `/statusline`, `/title`, `/permissions`, "always" answers at permission prompts, and `trustedWorkspaces`.
- Unknown keys are kept: "The CLI ignores keys it doesn't recognize, such as `$schema` or a setting from a newer version, and keeps them when it saves the file."
- No managed or system settings file is documented, and no variable moves the settings folder or home. Whether `~` is `USERPROFILE` or `HOME` on Windows is not documented; the live-check script sets `USERPROFILE`.
- Open: whether a running session overwrites a change made to the file while it runs (slice 6g's live check answers this).
