//! What a text tab's save needs from the core: which files it may write (`guard`), the write itself
//! (`write`), the copies of the files a save replaced (`history`), and the merge of a file that
//! changed on disk into the person's unsaved text (`merge`). These are the person's own
//! files, anywhere they can write, so nothing here goes through the config service, which is for
//! the changes Marshell itself proposes.

pub mod drafts;
pub mod guard;
pub mod history;
pub mod merge;
pub mod write;
