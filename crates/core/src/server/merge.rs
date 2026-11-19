//! `POST /v1/files/merge`: a text tab's three-way merge. The route takes three texts and no path,
//! and writes nothing: the merged text goes back to the editor, and reaches the disk only through
//! a save.
//!
//! A merge can be slow (see `userfiles::merge`), and a blocking task can't be stopped. So a merge
//! is given `MERGE_TIMEOUT`, and one runs at a time: a merge that outlived its time keeps the slot
//! until it really ends, and the next one is told so instead of piling up beside it.

use super::{AppState, drafts};
use crate::userfiles::merge::{self as texts, Merged, TooDifferent};
use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use marshell_protocol::api::{FileMerge, FileMerged};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Semaphore;

/// How long a merge may take before its request is answered without it.
pub const MERGE_TIMEOUT: Duration = Duration::from_secs(10);

const TEXT_TOO_LARGE: &str = "A text is over 3 MB.";
const TOO_DIFFERENT: &str = "The two versions differ in too many lines.";
const TOO_LONG: &str = "The merged text would be over 3 MB.";
const SLOW: &str = "The merge took too long.";
const BUSY: &str = "Another merge is still running.";

/// How a merge ended.
#[derive(Debug, PartialEq, Eq)]
enum Done {
    Merged(Merged),
    TooDifferent,
    /// The merged text is over the cap a draft has, so the tab could not keep it.
    TooLong,
    Slow,
    Busy,
    /// The work panicked.
    Failed,
}

/// Runs `work` off the async threads, alone in `slot`, for at most `limit`.
async fn run<F>(slot: Arc<Semaphore>, limit: Duration, work: F) -> Done
where
    F: FnOnce() -> Result<Merged, TooDifferent> + Send + 'static,
{
    let Ok(permit) = slot.try_acquire_owned() else {
        return Done::Busy;
    };
    // The permit goes with the work, so it is given back when the work ends: also when it
    // panicked, and when its time ran out long before.
    let task = tokio::task::spawn_blocking(move || {
        let _permit = permit;
        work()
    });
    match tokio::time::timeout(limit, task).await {
        Err(_) => Done::Slow,
        Ok(Err(_)) => Done::Failed,
        Ok(Ok(Err(TooDifferent))) => Done::TooDifferent,
        Ok(Ok(Ok(merged))) if merged.text.len() as u64 > drafts::DRAFT_TEXT_CAP => Done::TooLong,
        Ok(Ok(Ok(merged))) => Done::Merged(merged),
    }
}

/// POST /v1/files/merge: folds the disk's changes into the person's text.
pub async fn merge(State(st): State<AppState>, Json(req): Json<FileMerge>) -> Response {
    let over = |text: &String| text.len() as u64 > drafts::DRAFT_TEXT_CAP;
    if over(&req.base) || over(&req.mine) || over(&req.disk) {
        return (StatusCode::PAYLOAD_TOO_LARGE, TEXT_TOO_LARGE).into_response();
    }
    let work = move || texts::merge(&req.base, &req.mine, &req.disk);
    match run(st.0.merge_slot.clone(), MERGE_TIMEOUT, work).await {
        Done::Merged(Merged { text, conflicts }) => Json(FileMerged { text, conflicts }).into_response(),
        Done::TooDifferent => (StatusCode::UNPROCESSABLE_ENTITY, TOO_DIFFERENT).into_response(),
        Done::TooLong => (StatusCode::UNPROCESSABLE_ENTITY, TOO_LONG).into_response(),
        Done::Slow => (StatusCode::SERVICE_UNAVAILABLE, SLOW).into_response(),
        Done::Busy => (StatusCode::SERVICE_UNAVAILABLE, BUSY).into_response(),
        Done::Failed => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn slot() -> Arc<Semaphore> {
        Arc::new(Semaphore::new(1))
    }

    fn quick() -> Result<Merged, TooDifferent> {
        Ok(Merged {
            text: "a\n".into(),
            conflicts: 0,
        })
    }

    #[tokio::test]
    async fn a_merge_in_time_answers_its_text() {
        let slot = slot();
        assert_eq!(
            run(slot.clone(), Duration::from_secs(5), quick).await,
            Done::Merged(quick().unwrap())
        );
        // The slot is free again for the next one.
        assert_eq!(
            run(slot, Duration::from_secs(5), || Err(TooDifferent)).await,
            Done::TooDifferent
        );
    }

    #[tokio::test]
    async fn a_merge_past_the_time_limit_answers_slow_and_keeps_the_slot_until_it_ends() {
        let slot = slot();
        let slow = || {
            std::thread::sleep(Duration::from_millis(300));
            quick()
        };
        assert_eq!(run(slot.clone(), Duration::from_millis(50), slow).await, Done::Slow);
        // The slow one is still running.
        assert_eq!(run(slot.clone(), Duration::from_secs(5), quick).await, Done::Busy);
        tokio::time::sleep(Duration::from_millis(600)).await;
        assert_eq!(
            run(slot, Duration::from_secs(5), quick).await,
            Done::Merged(quick().unwrap())
        );
    }

    #[tokio::test]
    async fn a_merge_that_panics_frees_the_slot() {
        let slot = slot();
        assert_eq!(
            run(slot.clone(), Duration::from_secs(5), || panic!("the merge broke")).await,
            Done::Failed
        );
        assert_eq!(
            run(slot, Duration::from_secs(5), quick).await,
            Done::Merged(quick().unwrap())
        );
    }

    #[tokio::test]
    async fn a_result_over_the_cap_is_too_long() {
        let long = || {
            Ok(Merged {
                text: "x".repeat(drafts::DRAFT_TEXT_CAP as usize + 1),
                conflicts: 1,
            })
        };
        assert_eq!(run(slot(), Duration::from_secs(5), long).await, Done::TooLong);
    }
}
