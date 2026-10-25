//! Tool profiles: how Marshell recognises a developer tool (`vercel`, `terraform`, `gcloud`, ...) in a shell
//! and what it reads from the tool's output.

pub mod matcher;
pub mod own;
pub mod profile;

pub use marshell_protocol::tools::PortChip;
pub use marshell_protocol::tools::{
    LaunchPreset, NeedsYouPattern, OwnToolProblem, PortPattern, PresetField, PromptKind, ToolProfile,
};
pub use matcher::{Matcher, ToolEvent, normalize_port};
pub use profile::{Classified, CompiledNeedsYou, Profile, Tools, validate_own};
