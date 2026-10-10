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
