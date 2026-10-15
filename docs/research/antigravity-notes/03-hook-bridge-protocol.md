# Antigravity CLI: Hook Bridge Protocol

When the `marshell-hook` binary is invoked by Antigravity CLI, it receives context as a JSON object on `stdin`. It MUST output its result as a JSON object on `stdout`.

**Crucial:** All keys in the hook payloads use **camelCase**.

## 1. `PreToolUse` (Permission Requests)

### Stdin Payload (from Antigravity)
```json
{
  "conversationId": "uuid-here",
  "workspacePaths": ["/path/to/project"],
  "transcriptPath": "/path/to/.gemini/antigravity-cli/transcript.jsonl",
  "artifactDirectoryPath": "/path/to/.gemini/antigravity-cli/artifacts",
  "modelName": "Gemini 3.1 Pro (High)",
  "stepIdx": 19,
  "toolCall": {
    "name": "run_command",
    "args": {
      "CommandLine": "npm test"
    }
  }
}
```

### Translation to Marshell
In `Adapter::normalize(raw_event)`, map this to `SessionEvent::PermissionRequested`:
- `request_id`: Use `<conversationId>-<stepIdx>` to uniquely identify it.
- `tool`: `toolCall.name`.
- `detail`: Parse `toolCall.args` to derive `PermissionDetail::Bash`, `Edit`, `Write`, etc.

### Stdout Payload (from Marshell via `marshell-hook`)
If Marshell determines the action needs user approval, it blocks the hook execution and waits for the GUI decision. 
The hook must print the following JSON to `stdout` to return the decision to Antigravity:

```json
{
  "decision": "allow",
  "reason": "Approved by user via Marshell",
  "permissionOverrides": []
}
```
*Valid decisions:* `"allow"`, `"deny"`, `"ask"`, `"force_ask"`.
Marshell will typically output `"allow"` or `"deny"` based on the user's explicit interaction in the GUI.

## 2. `Stop` (Execution Loop Termination)

### Stdin Payload
```json
{
  "conversationId": "uuid-here",
  "executionNum": 1,
  "terminationReason": "model_stop",
  "error": "",
  "fullyIdle": true,
  "workspacePaths": [],
  "transcriptPath": "...",
  "artifactDirectoryPath": "...",
  "modelName": "auto"
}
```

### Translation to Marshell
Map this to `SessionEvent::TurnDone` or `TurnFailed` depending on `error`.

### Stdout Payload
To let it stop normally:
```json
{}
```
If Marshell is in "Goal" mode and wants the agent to continue, it can force-continue:
```json
{
  "decision": "continue",
  "reason": "The tests are still running in the background. Please wait."
}
```
