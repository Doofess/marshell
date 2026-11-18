//! What a text tab's save needs from the core: which files it may write (`guard`), the write itself
//! (`write`) and the copies of the files a save replaced (`history`). These are the person's own
//! files, anywhere they can write, so nothing here goes through the config service, which is for
//! the changes Marshell itself proposes.

pub mod guard;
pub mod history;
pub mod write;
