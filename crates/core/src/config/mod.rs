//! The config safety layer (PLAN §6). Every write Marshell makes to a CLI's configuration is a
//! ChangeSet that is previewed as a diff, applied only with the hashes the user saw, snapshotted,
//! written atomically, recorded in `managed.json`, and can be restored.

pub mod guard;
pub mod hash;
pub mod jsonc;
pub mod write;
