# Antigravity CLI: UI Design & Brand Tokens

When rendering the Antigravity CLI in the Marshell UI, adhere to the strict layout specs defined in `docs/PLAN.md` with these Antigravity-specific variables:

## Brand Identity
- **Agent Name:** `Antigravity`
- **Brand Accent:** Purple/Indigo. (Check if the user prefers `#6b21a8` or similar accessible indigo/purple, representing the Google Gemini/Antigravity lineage).
- **Mark/Icon:** A distinct mark (e.g., the Gemini star or a stylized 'A'). Must be drawn as a 16px SVG.

## Motion & Status Glyphs
According to `docs/PLAN.md`, the `Working` status glyph for Antigravity is explicitly defined as:
> *Antigravity swims up like a jellyfish, the bell squeezing and the tentacles trailing...*

You must implement this CSS keyframe animation for the Antigravity session row when the agent is in the `Working` state. The animation should pause and fall back gracefully for users with `prefers-reduced-motion`.

## Capabilities Flag
Ensure the adapter exposes the following capabilities as `true`:
- `hooks`
- `permission_hook` (Thanks to the robust `PreToolUse` contract)
- `history`

The `statusline` capability is `false`, as Antigravity does not feature an actively running chained status-line command natively out-of-the-box like Claude Code. Instead, rely on `Adapter::usage_source() -> UsageSource::Transcript`.
