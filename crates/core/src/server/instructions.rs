//! The instruction library's routes (slice 8a): what the Instructions page shows, and its saves, Review, deletes and
//! copy fixes. Every route sits behind the bearer token. No request names a path to write: a doc is named by its scope,
//! project and name, and a copy by those with its CLI and distro. The files are written by the config service, as the
//! person's own change, so nothing is recorded in `managed.json`. A stopped distro is never read.

use super::AppState;
use super::agents::found_agents;
use super::doctor::{managed_entries, partition_managed_distros};
use super::doctor_ext::windows_roots;
use crate::agents::codex::library::{Trust, project_trust, user_sets_developer_instructions};
use crate::agents::qwen::extensions::{folder_trust_on, folder_trusted};
use crate::config::jsonc::read_value;
use crate::config::service::ApplyError;
use crate::instructions::copy::{Disk, parse_end, parse_marker, read_disk};
use crate::instructions::doc::{self, CLIS, Doc};
use crate::instructions::library::{
    Level, LevelState, Notes, Place, delete_plan, level_state, put_back_plan, review_plan, save_plan, stays_inside,
    take_copy_plan,
};
use crate::instructions::roots::Roots;
use crate::projects;
use axum::Json;
use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use marshell_protocol::config::{
    ApplyConflict, ApplyRequest, ApplyResult, ChangeSet, ConfigPreview, ExpectedHash, FileStatus, RefuseReason,
};
use marshell_protocol::instructions::{
    CopyFix, CopyRow, CopyState, DocRef, DocSave, DocSaved, DocScope, DocView, LibraryReview, LibraryView,
};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

const NOT_KNOWN: &str = "Marshell doesn't know this project.";
const LINKED: &str = "This project's .marshell folder links outside it, so Marshell doesn't read its docs.";
const NO_DOC: &str = "No doc of that name here.";
const CHANGED: &str = "This copy changed. Check again.";
const NEEDS_PROJECT: &str = "A project doc needs a project.";
const CODEX_NO_ENTRY: &str = "Codex loads it once you trust this project in Codex.";
const CODEX_UNTRUSTED: &str = "Codex doesn't trust this project, so it loads none of its instructions.";
const QWEN_UNTRUSTED: &str = "Qwen Code loads it once you trust this folder in Qwen Code.";

/// Why a request stops: a sentence with its status, or an apply conflict with the files that changed.
enum Stop {
    Plain(StatusCode, String),
    Conflict(ApplyConflict),
}

impl Stop {
    fn plain(status: StatusCode, text: impl Into<String>) -> Stop {
        Stop::Plain(status, text.into())
    }

    fn unprocessable(text: impl Into<String>) -> Stop {
        Stop::plain(StatusCode::UNPROCESSABLE_ENTITY, text)
    }

    fn clash(text: impl Into<String>) -> Stop {
        Stop::plain(StatusCode::CONFLICT, text)
    }

    fn failed(err: impl std::fmt::Display) -> Stop {
        Stop::plain(StatusCode::INTERNAL_SERVER_ERROR, err.to_string())
    }

    fn changed(paths: Vec<String>) -> Stop {
        Stop::Conflict(ApplyConflict {
            reason: "changed outside Marshell".to_string(),
            paths,
        })
    }
}

impl IntoResponse for Stop {
    fn into_response(self) -> Response {
        match self {
            Stop::Plain(status, text) => (status, text).into_response(),
            Stop::Conflict(conflict) => (StatusCode::CONFLICT, Json(conflict)).into_response(),
        }
    }
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as u64)
}

/// Runs `work` off the async threads and answers its value as JSON.
async fn blocking<T, F>(st: AppState, work: F) -> Response
where
    T: Serialize + Send + 'static,
    F: FnOnce(&AppState) -> Result<T, Stop> + Send + 'static,
{
    match tokio::task::spawn_blocking(move || work(&st)).await {
        Ok(Ok(value)) => Json(value).into_response(),
        Ok(Err(stop)) => stop.into_response(),
        Err(e) => Stop::failed(e).into_response(),
    }
}

// ---- where things are ----

/// The running managed distros with their homes, and the managed distros that are not running. A stopped distro is
/// never asked for anything, since asking would start it. Runs `wsl.exe` when Marshell manages a distro.
pub(crate) fn library_distros(st: &AppState) -> (Vec<(String, PathBuf)>, Vec<String>) {
    let (running, stopped) = partition_managed_distros(st, &managed_entries(st));
    let homes = running
        .into_iter()
        .filter_map(|distro| {
            let target = st.distro_target(&distro).ok()?;
            Some((distro, target.home))
        })
        .collect();
    (homes, stopped)
}

/// The Windows user home, which a doc's source is shown under as `~/…`.
pub(crate) fn user_home(st: &AppState) -> PathBuf {
    windows_roots(st).map_or_else(|| st.0.home.clone(), |r| r.home)
}

fn read_user_docs(st: &AppState) -> Vec<Doc> {
    doc::read_dir(&st.0.home.join("instructions"), DocScope::User)
}

/// The CLIs found on a running distro's own PATH, in CLI order. Never called for a stopped distro.
fn found_in_distro(st: &AppState, distro: &str) -> Vec<&'static str> {
    let words = super::catalog::wsl_words(st);
    let asked: Vec<String> = words.iter().filter_map(|(_, word)| word.clone()).collect();
    let Some(found) = st.agents_in_distro(distro, &asked) else {
        return Vec::new();
    };
    CLIS.into_iter()
        .filter(|cli| {
            words
                .iter()
                .any(|(id, word)| id == cli && word.as_ref().is_some_and(|w| found.contains(w)))
        })
        .collect()
}

fn user_places_with(st: &AppState, docs: &[Doc], distro_homes: &[(String, PathBuf)]) -> Vec<Place> {
    let mut places = Vec::new();
    if let Some(roots) = windows_roots(st) {
        places.push(Place {
            level: Level::User {
                roots: Roots::from_ext(&roots),
                distro: None,
            },
            docs: docs.to_vec(),
            found: found_agents(st),
        });
    }
    for (distro, home) in distro_homes {
        places.push(Place {
            level: Level::User {
                roots: Roots::under(home),
                distro: Some(distro.clone()),
            },
            docs: docs.to_vec(),
            found: found_in_distro(st, distro),
        });
    }
    places
}

/// Every user level a doc is copied into: the Windows one first, then each running managed distro's (with the CLIs
/// found on its PATH). Reads the user docs. Blocks.
pub(crate) fn user_places(st: &AppState, distro_homes: &[(String, PathBuf)]) -> Vec<Place> {
    user_places_with(st, &read_user_docs(st), distro_homes)
}

fn read_text(path: &Path) -> Option<String> {
    match read_disk(path) {
        Disk::Text { text, .. } => Some(text),
        _ => None,
    }
}

/// Decision 17's sentences: whether Codex and Qwen Code load this project's files, from the user config where the
/// project is (Windows', or the running distro's). Nothing is shown without a home to read.
fn notes_for(st: &AppState, root: &Path, distro: Option<&str>, distro_homes: &[(String, PathBuf)]) -> Notes {
    let roots = match distro {
        None => windows_roots(st).map(|r| Roots::from_ext(&r)),
        Some(d) => distro_homes
            .iter()
            .find(|(name, _)| name.eq_ignore_ascii_case(d))
            .map(|(_, home)| Roots::under(home)),
    };
    let Some(roots) = roots else {
        return Notes::default();
    };
    // The key the CLI knows the folder by: its Windows path, or the distro's Linux path.
    let key_of = |path: &Path| match distro {
        None => Some(path.display().to_string()),
        Some(d) => crate::wsl::to_linux(d, path),
    };
    let Some(key) = key_of(root) else {
        return Notes::default();
    };
    let config = read_text(&roots.codex.join("config.toml"));
    let git_key = root
        .ancestors()
        .find(|a| key_of(a).is_some() && a.join(".git").exists())
        .and_then(key_of);
    let codex = match project_trust(config.as_deref(), &key, git_key.as_deref()) {
        Trust::Trusted => None,
        Trust::NoEntry => Some(CODEX_NO_ENTRY.to_string()),
        Trust::Untrusted => Some(CODEX_UNTRUSTED.to_string()),
    };
    let json = |name: &str| read_text(&roots.qwen.join(name)).and_then(|t| read_value(&t).ok());
    let settings = json("settings.json");
    let qwen = if folder_trust_on(settings.as_ref()) {
        let folder = match distro {
            None => root.to_path_buf(),
            Some(_) => PathBuf::from(&key),
        };
        (!folder_trusted(&json("trustedFolders.json"), &folder)).then(|| QWEN_UNTRUSTED.to_string())
    } else {
        None
    };
    Notes {
        codex,
        codex_user_warning: user_sets_developer_instructions(config.as_deref()),
        qwen,
    }
}

fn stopped_note(distro: &str) -> String {
    format!("{distro} isn't running, so this project's docs can't be read.")
}

/// A project's docs and where its copies go. `Err` holds the sentence for a project that isn't read: a distro that
/// isn't running, or a `.marshell` folder that links outside the project (a cloned repo must not make Marshell read,
/// or write, another folder).
pub(crate) fn project_place(st: &AppState, root: &Path, distro_homes: &[(String, PathBuf)]) -> Result<Place, String> {
    let distro = root.to_str().and_then(crate::wsl::share_distro).map(str::to_string);
    if let Some(d) = &distro {
        let running = (st.0.running_distros)();
        if !running.iter().any(|r| r.eq_ignore_ascii_case(d)) {
            return Err(stopped_note(d));
        }
    }
    read_project(st, root, distro.as_deref(), distro_homes)
}

fn read_project(
    st: &AppState,
    root: &Path,
    distro: Option<&str>,
    distro_homes: &[(String, PathBuf)],
) -> Result<Place, String> {
    let dir = root.join(".marshell").join("instructions");
    if !stays_inside(&dir, root) {
        return Err(LINKED.to_string());
    }
    Ok(Place {
        level: Level::Project {
            root: root.to_path_buf(),
            notes: notes_for(st, root, distro, distro_homes),
        },
        docs: doc::read_dir(&dir, DocScope::Project),
        found: match distro {
            None => found_agents(st),
            Some(d) => found_in_distro(st, d),
        },
    })
}

/// A project of the request: one of Marshell's, either read or with the reason it isn't.
enum Project {
    Ready { shown: String, place: Box<Place> },
    Unread { shown: String, note: String },
}

impl Project {
    fn shown(&self) -> &str {
        match self {
            Project::Ready { shown, .. } | Project::Unread { shown, .. } => shown,
        }
    }
}

/// The project whose root is `root`, among `projects::entries`. A distro that isn't running is never read: its project
/// is listed with `read == false`.
fn resolve(st: &AppState, root: &str, distro_homes: &[(String, PathBuf)]) -> Result<Project, Stop> {
    let listed = projects::entries(&st.0.home, &|| (st.0.running_distros)());
    let Some(found) = listed.into_iter().find(|l| projects::same_path(&l.info.path, root)) else {
        return Err(Stop::plain(StatusCode::NOT_FOUND, NOT_KNOWN));
    };
    let shown = found.info.path;
    let distro = crate::wsl::share_distro(&shown).map(str::to_string);
    if !found.read {
        let note = stopped_note(distro.as_deref().unwrap_or_default());
        return Ok(Project::Unread { shown, note });
    }
    Ok(
        match read_project(st, Path::new(&shown), distro.as_deref(), distro_homes) {
            Ok(place) => Project::Ready {
                shown,
                place: Box::new(place),
            },
            Err(note) => Project::Unread { shown, note },
        },
    )
}

/// The project's place for a request that writes: a project that isn't read answers 409 with its sentence.
fn ready_place(st: &AppState, root: &str, distro_homes: &[(String, PathBuf)]) -> Result<Place, Stop> {
    match resolve(st, root, distro_homes)? {
        Project::Ready { place, .. } => Ok(*place),
        Project::Unread { note, .. } => Err(Stop::clash(note)),
    }
}

// ---- the view ----

fn waiting_row(distro: &str) -> CopyRow {
    CopyRow {
        cli: None,
        distro: Some(distro.to_string()),
        path: None,
        state: CopyState::Waiting,
        notes: vec![format!("{distro} isn't running, so its copies wait.")],
        shared: false,
        fixes: false,
    }
}

/// The rows of the doc at `index` in each level's state, in order.
fn rows_at(states: &[LevelState], index: usize) -> Vec<CopyRow> {
    states
        .iter()
        .filter_map(|s| s.rows.get(index))
        .flat_map(|(_, rows)| rows.iter().cloned())
        .collect()
}

fn build_view(
    st: &AppState,
    project: Option<&str>,
    distro_homes: &[(String, PathBuf)],
    stopped: &[String],
) -> Result<LibraryView, Stop> {
    let docs = read_user_docs(st);
    let places = user_places_with(st, &docs, distro_homes);
    let home = user_home(st);
    let editable = |p: &Path| st.0.config.editable(p);
    let states: Vec<LevelState> = places.iter().map(|p| level_state(p, &home, &editable)).collect();
    let user_docs = docs
        .iter()
        .enumerate()
        .map(|(i, d)| {
            let mut rows = rows_at(&states, i);
            rows.extend(stopped.iter().map(|s| waiting_row(s)));
            DocView {
                doc: doc::to_protocol(d),
                rows,
            }
        })
        .collect();
    let mut view = LibraryView {
        project: None,
        project_note: None,
        user_docs,
        project_docs: Vec::new(),
        user_orphans: states.iter().flat_map(|s| s.orphans.iter().cloned()).collect(),
        project_orphans: Vec::new(),
        out_of_date: states.iter().map(|s| s.out_of_date).sum(),
        changed_outside: states.iter().map(|s| s.changed_outside).sum(),
        found: found_agents(st).into_iter().map(str::to_string).collect(),
    };
    if let Some(root) = project {
        let resolved = resolve(st, root, distro_homes)?;
        view.project = Some(resolved.shown().to_string());
        match resolved {
            Project::Unread { note, .. } => view.project_note = Some(note),
            Project::Ready { place, .. } => {
                let state = level_state(&place, &home, &editable);
                view.project_docs = place
                    .docs
                    .iter()
                    .enumerate()
                    .map(|(i, d)| DocView {
                        doc: doc::to_protocol(d),
                        rows: rows_at(std::slice::from_ref(&state), i),
                    })
                    .collect();
                view.out_of_date += state.out_of_date;
                view.changed_outside += state.changed_outside;
                view.project_orphans = state.orphans;
            }
        }
    }
    Ok(view)
}

#[derive(Deserialize)]
pub struct ViewQuery {
    project: Option<String>,
}

/// `GET /v1/instructions?project=<root>`.
pub async fn library(State(st): State<AppState>, Query(q): Query<ViewQuery>) -> Response {
    blocking(st, move |st| {
        let (homes, stopped) = library_distros(st);
        build_view(st, q.project.as_deref(), &homes, &stopped)
    })
    .await
}

// ---- saving ----

/// The first line of `body` that Marshell's copies use as a marker, counted from 1.
fn marker_line(body: &str) -> Option<usize> {
    body.lines()
        .position(|line| parse_marker(line).is_some() || parse_end(line).is_some())
        .map(|i| i + 1)
}

/// What a person's save of a doc turns into, checked: the doc as it will be written and the doc it replaces.
fn doc_of<'a>(req: &DocSave, dir: &Path, docs: &'a [Doc]) -> Result<(Doc, Option<&'a Doc>), Stop> {
    let mut clis = Vec::new();
    for id in &req.clis {
        match CLIS.iter().find(|c| *c == id) {
            Some(c) => clis.push(*c),
            None => return Err(Stop::unprocessable(format!("{id} isn't a CLI Marshell knows."))),
        }
    }
    let old = match &req.old_name {
        Some(name) => Some(doc::find(docs, name).ok_or_else(|| Stop::plain(StatusCode::NOT_FOUND, NO_DOC))?),
        None => None,
    };
    // A doc Marshell refused to read (a link, or over 4 MiB) is never written over: a save would go wherever it points.
    if let Some(Err(why)) = old.map(|o| doc::read_plain(&o.path)) {
        return Err(Stop::clash(why));
    }
    let taken: Vec<&str> = docs
        .iter()
        .filter(|d| !old.is_some_and(|o| std::ptr::eq(*d, o)))
        .map(|d| d.name.as_str())
        .collect();
    doc::check_name(&req.name, &taken).map_err(Stop::unprocessable)?;
    if old.is_some_and(|o| o.name != req.name && o.name.eq_ignore_ascii_case(&req.name)) {
        return Err(Stop::unprocessable("A new name must differ by more than case."));
    }
    let globs = doc::check_globs(&req.globs).map_err(Stop::unprocessable)?;
    doc::check_description(&req.description).map_err(Stop::unprocessable)?;
    let body = req.body.replace("\r\n", "\n");
    if let Some(n) = marker_line(&body) {
        return Err(Stop::unprocessable(format!(
            "Line {n} marks Marshell's copies, so a doc can't hold it."
        )));
    }
    // A doc keeps its own file when its name doesn't change; a new name is joined only now that it passed.
    let path = match old {
        Some(o) if o.name == req.name => o.path.clone(),
        _ => {
            let path = dir.join(format!("{}.md", req.name));
            if std::fs::symlink_metadata(&path).is_ok() {
                return Err(Stop::clash(format!("A file named {}.md is already there.", req.name)));
            }
            path
        }
    };
    let doc = Doc {
        name: req.name.clone(),
        scope: req.scope,
        path,
        description: req.description.clone(),
        clis: CLIS.into_iter().filter(|c| clis.contains(c)).collect(),
        globs,
        enabled: req.scope == DocScope::User || req.enabled,
        body,
        extra: old.map(|o| o.extra.clone()).unwrap_or_default(),
        hash: String::new(),
        error: None,
        duplicate: false,
    };
    Ok((doc, old))
}

/// The hashes a preview must be applied with: the ones it showed.
fn shown_hashes(preview: &ConfigPreview) -> ApplyRequest {
    ApplyRequest {
        changeset_id: preview.changeset_id.clone(),
        expected_hashes: preview
            .files
            .iter()
            .map(|f| ExpectedHash {
                path: f.path.clone(),
                hash: f.before_hash.clone(),
            })
            .collect(),
    }
}

/// Previews a change as the person's own and applies it at once (decision 39), as `/v1/config/apply` would: a file that
/// changed since it was read is a 409 with the files named, and nothing is written.
fn apply_now(st: &AppState, cs: ChangeSet) -> Result<ApplyResult, Stop> {
    let preview = preview_of(st, cs);
    for file in &preview.files {
        if let FileStatus::Refused { reason, detail } = &file.status {
            return Err(match reason {
                RefuseReason::Conflict => Stop::changed(vec![file.path.clone()]),
                _ => Stop::unprocessable(detail.clone()),
            });
        }
    }
    if preview.files.iter().all(|f| f.status != FileStatus::Changes) {
        return Err(Stop::unprocessable("Nothing changes."));
    }
    match st.0.config.apply(&shown_hashes(&preview), now_ms()) {
        Ok(result) => Ok(result),
        Err(ApplyError::Conflict { reason, paths }) => Err(Stop::Conflict(ApplyConflict { reason, paths })),
        Err(ApplyError::UnknownChangeSet) => Err(Stop::changed(Vec::new())),
        Err(ApplyError::Failed(e)) => Err(Stop::failed(format!("{e:#}"))),
    }
}

fn save_doc(st: &AppState, req: DocSave) -> Result<DocSaved, Stop> {
    let (homes, stopped) = library_distros(st);
    if req.scope == DocScope::Project && req.project.is_none() {
        return Err(Stop::unprocessable(NEEDS_PROJECT));
    }
    // The page's project must be one of Marshell's whichever scope is saved; only a project doc needs it read.
    let project = req.project.as_deref().map(|p| resolve(st, p, &homes)).transpose()?;
    let (dir, docs) = match (req.scope, project) {
        (DocScope::User, _) => (st.0.home.join("instructions"), read_user_docs(st)),
        (DocScope::Project, Some(Project::Ready { place, .. })) => match &place.level {
            Level::Project { root, .. } => (root.join(".marshell").join("instructions"), place.docs.clone()),
            Level::User { .. } => return Err(Stop::failed("a project's place is a project level")),
        },
        (DocScope::Project, Some(Project::Unread { note, .. })) => return Err(Stop::clash(note)),
        (DocScope::Project, None) => return Err(Stop::unprocessable(NEEDS_PROJECT)),
    };
    let (doc, old) = doc_of(&req, &dir, &docs)?;
    let applied = apply_now(st, save_plan(&doc, old, req.expected_hash.as_deref()))?;
    Ok(DocSaved {
        snapshot_id: applied.snapshot_id,
        view: build_view(st, req.project.as_deref(), &homes, &stopped)?,
    })
}

/// `POST /v1/instructions/save`.
pub async fn save(State(st): State<AppState>, Json(req): Json<DocSave>) -> Response {
    blocking(st, move |st| save_doc(st, req)).await
}

// ---- review, delete and the copy fixes ----

fn preview_of(st: &AppState, cs: ChangeSet) -> ConfigPreview {
    st.0.config.preview_own(cs, &|_| None)
}

fn review_copies(st: &AppState, req: LibraryReview) -> Result<ConfigPreview, Stop> {
    let (homes, _) = library_distros(st);
    let project = req.project.as_deref().map(|p| ready_place(st, p, &homes)).transpose()?;
    let mut places = user_places(st, &homes);
    places.extend(project);
    let editable = |p: &Path| st.0.config.editable(p);
    Ok(preview_of(st, review_plan(&places, &user_home(st), &editable)))
}

/// `POST /v1/instructions/review`.
pub async fn review(State(st): State<AppState>, Json(req): Json<LibraryReview>) -> Response {
    blocking(st, move |st| review_copies(st, req)).await
}

fn delete_doc(st: &AppState, req: DocRef) -> Result<ConfigPreview, Stop> {
    let (homes, _) = library_distros(st);
    let places = match req.scope {
        DocScope::User => user_places(st, &homes),
        DocScope::Project => {
            let root = req
                .project
                .as_deref()
                .ok_or_else(|| Stop::unprocessable(NEEDS_PROJECT))?;
            vec![ready_place(st, root, &homes)?]
        }
    };
    let doc = places
        .iter()
        .find_map(|p| doc::find(&p.docs, &req.name))
        .ok_or_else(|| Stop::plain(StatusCode::NOT_FOUND, NO_DOC))?;
    let editable = |p: &Path| st.0.config.editable(p);
    let cs = delete_plan(&places, &doc.name, &user_home(st), &editable);
    Ok(preview_of(st, cs))
}

/// `POST /v1/instructions/delete`.
pub async fn delete(State(st): State<AppState>, Json(req): Json<DocRef>) -> Response {
    blocking(st, move |st| delete_doc(st, req)).await
}

fn fix_copy(st: &AppState, req: CopyFix) -> Result<ConfigPreview, Stop> {
    let (homes, stopped) = library_distros(st);
    let place = match req.scope {
        DocScope::Project => {
            let root = req
                .project
                .as_deref()
                .ok_or_else(|| Stop::unprocessable(NEEDS_PROJECT))?;
            ready_place(st, root, &homes)?
        }
        DocScope::User => {
            let wanted = req.distro.as_deref();
            let found = user_places(st, &homes).into_iter().find(|p| match &p.level {
                Level::User { distro, .. } => match (distro, wanted) {
                    (None, None) => true,
                    (Some(a), Some(b)) => a.eq_ignore_ascii_case(b),
                    _ => false,
                },
                Level::Project { .. } => false,
            });
            match (found, wanted) {
                (Some(place), _) => place,
                (None, Some(d)) if stopped.iter().any(|s| s.eq_ignore_ascii_case(d)) => {
                    return Err(Stop::clash(format!("{d} isn't running, so its copies wait.")));
                }
                (None, _) => return Err(Stop::clash(CHANGED)),
            }
        }
    };
    let doc = doc::find(&place.docs, &req.name).ok_or_else(|| Stop::plain(StatusCode::NOT_FOUND, NO_DOC))?;
    let editable = |p: &Path| st.0.config.editable(p);
    let home = user_home(st);
    let plan = if req.take_copy {
        take_copy_plan(&place, &doc.name, &req.cli, &home, &editable)
    } else {
        put_back_plan(&place, &doc.name, &req.cli, &home, &editable)
    };
    let cs = plan.ok_or_else(|| Stop::clash(CHANGED))?;
    Ok(preview_of(st, cs))
}

/// `POST /v1/instructions/copy`.
pub async fn copy(State(st): State<AppState>, Json(req): Json<CopyFix>) -> Response {
    blocking(st, move |st| fix_copy(st, req)).await
}

#[cfg(test)]
mod tests {
    use crate::config::managed::{self, ManagedEntry};
    use crate::instructions::doc::CLIS;
    use crate::projects::{canonical, record};
    use crate::server::{AppState, DistroTarget, router};
    use axum::body::Body;
    use axum::http::{Request, StatusCode, header};
    use marshell_protocol::api::{Profile, ShellFamily};
    use marshell_protocol::config::{ApplyRequest, ConfigPreview, ExpectedHash};
    use marshell_protocol::instructions::{CopyState, DocSave, DocSaved, LibraryView};
    use std::fs;
    use std::path::{Path, PathBuf};
    use tower::ServiceExt;

    const TOKEN: &str = "test-token";
    const PORT: u16 = 5571;
    const DISTRO: &str = "MarshellTestDistro";

    struct Rig {
        _dir: tempfile::TempDir,
        st: AppState,
        home: PathBuf,
        ext: PathBuf,
        project: PathBuf,
    }

    fn only_three(name: &str) -> Option<PathBuf> {
        matches!(name.trim_end_matches(".exe"), "claude" | "codex" | "copilot")
            .then(|| PathBuf::from(format!(r"Z:\nowhere\{name}.exe")))
    }

    fn wsl_profile() -> Profile {
        Profile {
            id: "test-distro".into(),
            name: DISTRO.into(),
            command: vec!["wsl.exe".into(), "-d".into(), DISTRO.into()],
            family: ShellFamily::Wsl,
            cwd: None,
            env: Vec::new(),
            shell_integration: false,
            terminal_theme: None,
            detected: true,
        }
    }

    /// A Marshell home, a Windows user home (`ext`) and one project, with `claude`, `codex` and `copilot` found.
    fn rig_with(profiles: Vec<Profile>, shape: impl FnOnce(AppState) -> AppState) -> Rig {
        let dir = tempfile::tempdir().unwrap();
        let root = canonical(dir.path());
        let home = root.join("home");
        let ext = root.join("ext");
        let project = root.join("repo");
        for d in [&home, &ext, &project] {
            std::fs::create_dir_all(d).unwrap();
        }
        record(&home, &project, 1).unwrap();
        let st = AppState::new(TOKEN.into(), PORT, Vec::new(), home.clone(), profiles)
            .with_ext_home(ext.clone())
            .with_agent_path(only_three)
            .with_running_distros(Vec::new);
        Rig {
            _dir: dir,
            st: shape(st),
            home,
            ext,
            project,
        }
    }

    fn rig() -> Rig {
        rig_with(Vec::new(), |st| st)
    }

    fn write(path: &Path, text: &str) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    }

    fn doc_text(description: &str, body: &str) -> String {
        format!(
            "---\ndescription: '{description}'\nclis: [claude, codex, antigravity, qwen, copilot]\nglobs: []\n---\n{body}"
        )
    }

    fn enc(s: &str) -> String {
        s.bytes()
            .map(|b| {
                if b.is_ascii_alphanumeric() || b"-._~".contains(&b) {
                    (b as char).to_string()
                } else {
                    format!("%{b:02X}")
                }
            })
            .collect()
    }

    async fn call(
        st: &AppState,
        method: &str,
        uri: &str,
        body: Option<serde_json::Value>,
        bearer: bool,
    ) -> (StatusCode, String) {
        let mut req = Request::builder()
            .method(method)
            .uri(uri)
            .header(header::HOST, format!("127.0.0.1:{PORT}"))
            .header(header::CONTENT_TYPE, "application/json");
        if bearer {
            req = req.header(header::AUTHORIZATION, format!("Bearer {TOKEN}"));
        }
        let body = body.map_or_else(Body::empty, |b| Body::from(b.to_string()));
        let res = router(st.clone()).oneshot(req.body(body).unwrap()).await.unwrap();
        let status = res.status();
        let bytes = axum::body::to_bytes(res.into_body(), 1 << 22).await.unwrap();
        (status, String::from_utf8_lossy(&bytes).into_owned())
    }

    async fn view(st: &AppState, project: Option<&Path>) -> (StatusCode, String) {
        let uri = match project {
            Some(p) => format!("/v1/instructions?project={}", enc(&p.to_string_lossy())),
            None => "/v1/instructions".to_string(),
        };
        call(st, "GET", &uri, None, true).await
    }

    async fn view_ok(st: &AppState, project: Option<&Path>) -> LibraryView {
        let (status, body) = view(st, project).await;
        assert_eq!(status, StatusCode::OK, "{body}");
        serde_json::from_str(&body).unwrap()
    }

    fn save_of(name: &str, body: &str) -> DocSave {
        DocSave {
            scope: marshell_protocol::instructions::DocScope::User,
            project: None,
            name: name.into(),
            old_name: None,
            description: "d".into(),
            clis: vec!["claude".into(), "codex".into()],
            globs: Vec::new(),
            enabled: true,
            body: body.into(),
            expected_hash: None,
        }
    }

    async fn post<T: serde::Serialize>(st: &AppState, route: &str, body: &T) -> (StatusCode, String) {
        call(
            st,
            "POST",
            &format!("/v1/instructions/{route}"),
            Some(serde_json::to_value(body).unwrap()),
            true,
        )
        .await
    }

    fn apply(st: &AppState, preview: &ConfigPreview) {
        let req = ApplyRequest {
            changeset_id: preview.changeset_id.clone(),
            expected_hashes: preview
                .files
                .iter()
                .map(|f| ExpectedHash {
                    path: f.path.clone(),
                    hash: f.before_hash.clone(),
                })
                .collect(),
        };
        st.0.config
            .apply(&req, 1)
            .unwrap_or_else(|e| panic!("apply failed: {e:?}"));
    }

    async fn review_and_apply(st: &AppState) {
        let (status, body) = post(st, "review", &serde_json::json!({})).await;
        assert_eq!(status, StatusCode::OK, "{body}");
        apply(st, &serde_json::from_str(&body).unwrap());
    }

    #[tokio::test]
    async fn view_lists_user_and_project_docs() {
        let r = rig();
        write(
            &r.home.join("instructions").join("security.md"),
            &doc_text("s", "Be careful.\n"),
        );
        write(
            &r.project.join(".marshell").join("instructions").join("api.md"),
            &doc_text("a", "Use the API.\n"),
        );
        let v = view_ok(&r.st, Some(&r.project)).await;
        assert_eq!(v.user_docs.len(), 1);
        assert_eq!(v.project_docs.len(), 1);
        assert_eq!(v.project_note, None);
        assert_eq!(v.found, ["claude", "codex", "copilot"]);
        for d in v.user_docs.iter().chain(&v.project_docs) {
            let clis: Vec<&str> = d.rows.iter().map(|x| x.cli.as_deref().unwrap()).collect();
            assert_eq!(clis, CLIS, "{}", d.doc.name);
        }
        let rows = &v.user_docs[0].rows;
        for cli in ["antigravity", "qwen"] {
            let row = rows.iter().find(|x| x.cli.as_deref() == Some(cli)).unwrap();
            assert_eq!(row.state, CopyState::NoCopy);
            assert_eq!(row.notes, ["Not installed on this PC, so it gets no copy."]);
        }
    }

    #[tokio::test]
    async fn unknown_project_is_404() {
        let r = rig();
        let (status, body) = view(&r.st, Some(&r.home.join("nowhere"))).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body, "Marshell doesn't know this project.");
    }

    #[tokio::test]
    async fn save_creates_the_folder_and_the_doc() {
        let r = rig();
        let (status, body) = post(&r.st, "save", &save_of("security", "Be careful.\n")).await;
        assert_eq!(status, StatusCode::OK, "{body}");
        let saved: DocSaved = serde_json::from_str(&body).unwrap();
        let text = std::fs::read_to_string(r.home.join("instructions").join("security.md")).unwrap();
        assert_eq!(
            text,
            "---\ndescription: 'd'\nclis: [claude, codex]\nglobs: []\n---\nBe careful.\n"
        );
        assert!(saved.view.out_of_date > 0, "{:?}", saved.view);
        assert_eq!(saved.view.user_docs.len(), 1);
        assert!(
            managed::list(&r.home)
                .unwrap()
                .iter()
                .all(|e| !e.path.contains("security")),
            "a save records nothing in managed.json"
        );
    }

    #[tokio::test]
    async fn save_with_a_stale_hash_conflicts() {
        let r = rig();
        let file = r.home.join("instructions").join("security.md");
        write(&file, &doc_text("s", "Old.\n"));
        let mut req = save_of("security", "New.\n");
        req.old_name = Some("security".into());
        req.expected_hash = Some("0123456789abcdef".into());
        let (status, body) = post(&r.st, "save", &req).await;
        assert_eq!(status, StatusCode::CONFLICT, "{body}");
        let conflict: marshell_protocol::config::ApplyConflict = serde_json::from_str(&body).unwrap();
        assert_eq!(conflict.paths.len(), 1, "{body}");
        assert!(std::fs::read_to_string(&file).unwrap().contains("Old."));
    }

    #[tokio::test]
    async fn save_refuses_a_bad_name() {
        let r = rig();
        let (status, body) = post(&r.st, "save", &save_of("bad name", "x\n")).await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(body, "A name holds letters, digits, - and _ only.");
    }

    #[tokio::test]
    async fn save_refuses_a_body_holding_a_marker_line() {
        let r = rig();
        let body = "One.\nTwo.\n<!-- marshell:end doc security -->\n";
        let (status, text) = post(&r.st, "save", &save_of("security", body)).await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(text, "Line 3 marks Marshell's copies, so a doc can't hold it.");
        assert!(!r.home.join("instructions").join("security.md").exists());
    }

    #[tokio::test]
    async fn delete_of_an_unknown_name_is_404() {
        let r = rig();
        let req = serde_json::json!({"scope": "user", "name": "nope"});
        let (status, body) = post(&r.st, "delete", &req).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body, "No doc of that name here.");
    }

    #[tokio::test]
    async fn review_then_apply_is_in_sync() {
        let r = rig();
        write(
            &r.home.join("instructions").join("security.md"),
            &doc_text("s", "Be careful.\n"),
        );
        assert!(view_ok(&r.st, None).await.out_of_date > 0);
        review_and_apply(&r.st).await;
        let v = view_ok(&r.st, None).await;
        assert_eq!(v.out_of_date, 0, "{v:?}");
        assert_eq!(v.changed_outside, 0);
        assert!(
            r.ext
                .join(".claude")
                .join("rules")
                .join("marshell-security.md")
                .exists()
        );
    }

    #[tokio::test]
    async fn review_records_nothing_in_managed_json() {
        let r = rig();
        write(
            &r.home.join("instructions").join("security.md"),
            &doc_text("s", "Be careful.\n"),
        );
        review_and_apply(&r.st).await;
        assert!(managed::list(&r.home).unwrap().is_empty());
    }

    #[tokio::test]
    async fn delete_preview_holds_the_doc_and_copies() {
        let r = rig();
        let doc = r.home.join("instructions").join("security.md");
        write(&doc, &doc_text("s", "Be careful.\n"));
        review_and_apply(&r.st).await;
        let req = serde_json::json!({"scope": "user", "name": "Security"});
        let (status, body) = post(&r.st, "delete", &req).await;
        assert_eq!(status, StatusCode::OK, "{body}");
        let preview: ConfigPreview = serde_json::from_str(&body).unwrap();
        let ends = |tail: &[&str]| {
            preview.files.iter().any(|f| {
                let p = Path::new(&f.path);
                let tail: PathBuf = tail.iter().collect();
                p.ends_with(tail)
            })
        };
        assert!(ends(&["instructions", "security.md"]), "{:?}", preview.files);
        assert!(
            ends(&[".claude", "rules", "marshell-security.md"]),
            "{:?}",
            preview.files
        );
        assert!(doc.exists(), "a preview writes nothing");
    }

    #[tokio::test]
    async fn copy_fix_of_an_in_sync_copy_conflicts() {
        let r = rig();
        write(
            &r.home.join("instructions").join("security.md"),
            &doc_text("s", "Be careful.\n"),
        );
        review_and_apply(&r.st).await;
        for take_copy in [false, true] {
            let req = serde_json::json!({"scope": "user", "name": "security", "cli": "claude", "take_copy": take_copy});
            let (status, body) = post(&r.st, "copy", &req).await;
            assert_eq!(status, StatusCode::CONFLICT, "{body}");
            assert_eq!(body, "This copy changed. Check again.");
        }
    }

    fn stopped_project(r: &Rig) -> String {
        let share = format!(r"\\wsl.localhost\{DISTRO}\home\u\repo");
        record(&r.home, Path::new(&share), 2).unwrap();
        share
    }

    #[tokio::test]
    async fn stopped_distro_project_reads_nothing() {
        let r = rig_with(vec![wsl_profile()], |st| st);
        let share = stopped_project(&r);
        let v = view_ok(&r.st, Some(Path::new(&share))).await;
        assert_eq!(
            v.project_note.as_deref(),
            Some("MarshellTestDistro isn't running, so this project's docs can't be read.")
        );
        assert!(v.project_docs.is_empty() && v.project_orphans.is_empty());
        assert_eq!(v.project.as_deref(), Some(share.as_str()));
    }

    #[tokio::test]
    async fn stopped_distro_project_refuses_writes() {
        let r = rig_with(vec![wsl_profile()], |st| st);
        let share = stopped_project(&r);
        let sentence = "MarshellTestDistro isn't running, so this project's docs can't be read.";
        let mut save = save_of("api", "x\n");
        save.scope = marshell_protocol::instructions::DocScope::Project;
        save.project = Some(share.clone());
        let doc = serde_json::json!({"scope": "project", "project": share, "name": "api"});
        let copy = serde_json::json!({"scope": "project", "project": share, "name": "api", "cli": "claude", "take_copy": false});
        let results = [
            post(&r.st, "save", &save).await,
            post(&r.st, "review", &serde_json::json!({"project": share})).await,
            post(&r.st, "delete", &doc).await,
            post(&r.st, "copy", &copy).await,
        ];
        for (status, body) in results {
            assert_eq!(status, StatusCode::CONFLICT, "{body}");
            assert_eq!(body, sentence);
        }
    }

    fn managed_in(r: &Rig, path: &str) {
        managed::save(
            &r.home,
            &[ManagedEntry {
                path: path.into(),
                pointer: Some("/hooks/Stop/-".into()),
                changeset_id: "c".into(),
                applied_ms: 1,
            }],
        )
        .unwrap();
    }

    #[tokio::test]
    async fn stopped_managed_distro_gets_a_waiting_row() {
        let r = rig_with(vec![wsl_profile()], |st| st);
        managed_in(&r, r"\\wsl.localhost\MarshellTestDistro\home\u\.claude\settings.json");
        write(
            &r.home.join("instructions").join("security.md"),
            &doc_text("s", "Be careful.\n"),
        );
        let v = view_ok(&r.st, None).await;
        for d in &v.user_docs {
            let waiting: Vec<_> = d.rows.iter().filter(|x| x.state == CopyState::Waiting).collect();
            assert_eq!(waiting.len(), 1, "{:?}", d.rows);
            assert_eq!(waiting[0].cli, None);
            assert_eq!(waiting[0].distro.as_deref(), Some(DISTRO));
            assert_eq!(
                waiting[0].notes,
                ["MarshellTestDistro isn't running, so its copies wait."]
            );
        }
    }

    #[tokio::test]
    async fn running_distro_copies_only_found_clis() {
        let dir = tempfile::tempdir().unwrap();
        let dhome = canonical(dir.path()).join("distro-home");
        std::fs::create_dir_all(&dhome).unwrap();
        let target = DistroTarget {
            home: dhome.clone(),
            bridge: "/mnt/c/marshell-hook.exe".into(),
        };
        let r = rig_with(vec![wsl_profile()], |st| {
            st.with_running_distros(|| vec![DISTRO.to_string()])
                .with_distro_target(Some(target))
                .with_distro_which(|_, words| Some(words.iter().filter(|w| w.as_str() == "claude").cloned().collect()))
        });
        managed_in(&r, &dhome.join(".claude").join("settings.json").to_string_lossy());
        let doc = r.home.join("instructions").join("security.md");
        write(&doc, &doc_text("s", "Be careful.\n"));
        let (status, body) = post(&r.st, "review", &serde_json::json!({})).await;
        assert_eq!(status, StatusCode::OK, "{body}");
        let preview: ConfigPreview = serde_json::from_str(&body).unwrap();
        let in_distro: Vec<&str> = preview
            .files
            .iter()
            .map(|f| f.path.as_str())
            .filter(|p| Path::new(p).starts_with(&dhome))
            .collect();
        assert_eq!(in_distro.len(), 1, "{:?}", preview.files);
        let want: PathBuf = [".claude", "rules", "marshell-security.md"].iter().collect();
        assert!(Path::new(in_distro[0]).ends_with(want), "{in_distro:?}");
        // The Windows level is planned too, and no distro copy is waiting.
        let v = view_ok(&r.st, None).await;
        assert!(v.user_docs[0].rows.iter().all(|x| x.state != CopyState::Waiting));
        drop(r);
    }

    fn link_dir(link: &Path, target: &Path) -> std::io::Result<()> {
        #[cfg(unix)]
        return std::os::unix::fs::symlink(target, link);
        #[cfg(windows)]
        return std::os::windows::fs::symlink_dir(target, link).or_else(|_| {
            let status = std::process::Command::new("cmd")
                .args(["/C", "mklink", "/J"])
                .arg(link)
                .arg(target)
                .output()?
                .status;
            status
                .success()
                .then_some(())
                .ok_or_else(|| std::io::Error::other("mklink /J failed"))
        });
    }

    #[tokio::test]
    async fn a_linked_marshell_folder_is_never_read_nor_written() {
        let r = rig();
        let outside = r.home.join("outside");
        write(
            &outside.join("instructions").join("secret.md"),
            &doc_text("x", "Private.\n"),
        );
        if let Err(e) = link_dir(&r.project.join(".marshell"), &outside) {
            eprintln!("skipped: no folder link here: {e}");
            return;
        }
        let sentence = "This project's .marshell folder links outside it, so Marshell doesn't read its docs.";
        let v = view_ok(&r.st, Some(&r.project)).await;
        assert_eq!(v.project_note.as_deref(), Some(sentence));
        assert!(v.project_docs.is_empty() && v.project_orphans.is_empty());
        let project = r.project.to_string_lossy().into_owned();
        let mut save = save_of("api", "x\n");
        save.scope = marshell_protocol::instructions::DocScope::Project;
        save.project = Some(project.clone());
        let (status, body) = post(&r.st, "save", &save).await;
        assert_eq!((status, body.as_str()), (StatusCode::CONFLICT, sentence));
        let (status, body) = post(&r.st, "review", &serde_json::json!({"project": project})).await;
        assert_eq!((status, body.as_str()), (StatusCode::CONFLICT, sentence));
        assert!(!outside.join("instructions").join("api.md").exists());
    }

    #[tokio::test]
    async fn a_linked_doc_is_never_saved_over() {
        let r = rig();
        let target = r.home.join("elsewhere");
        fs::create_dir_all(&target).unwrap();
        let dir = r.st.0.home.join("instructions");
        fs::create_dir_all(&dir).unwrap();
        if let Err(e) = link_dir(&dir.join("security.md"), &target) {
            eprintln!("skipped: no folder link here: {e}");
            return;
        }
        let v = view_ok(&r.st, None).await;
        let listed = &v.user_docs[0].doc;
        assert_eq!(listed.name, "security");
        let sentence = "This doc is a link to another file, so Marshell doesn't read it.";
        assert_eq!(listed.error.as_deref(), Some(sentence));
        let mut save = save_of("security", "Mine.\n");
        save.old_name = Some("security".into());
        save.expected_hash = Some(listed.hash.clone());
        let (status, body) = post(&r.st, "save", &save).await;
        assert_eq!((status, body.as_str()), (StatusCode::CONFLICT, sentence));
        assert!(fs::symlink_metadata(dir.join("security.md")).is_ok());
        assert_eq!(fs::read_dir(&target).unwrap().count(), 0);
    }

    #[tokio::test]
    async fn codex_trust_notes_come_from_the_users_config() {
        let r = rig();
        write(
            &r.project.join(".marshell").join("instructions").join("api.md"),
            &doc_text("a", "Use the API.\n"),
        );
        let codex_note = |v: &LibraryView| {
            let row = v.project_docs[0]
                .rows
                .iter()
                .find(|x| x.cli.as_deref() == Some("codex"))
                .unwrap();
            row.notes.join(" | ")
        };
        let v = view_ok(&r.st, Some(&r.project)).await;
        assert!(
            codex_note(&v).contains("Codex loads it once you trust this project in Codex."),
            "{}",
            codex_note(&v)
        );
        let key = r.project.to_string_lossy().replace('\\', "/");
        write(
            &r.ext.join(".codex").join("config.toml"),
            &format!("[projects.'{key}']\ntrust_level = \"untrusted\"\n"),
        );
        let v = view_ok(&r.st, Some(&r.project)).await;
        assert!(
            codex_note(&v).contains("Codex doesn't trust this project, so it loads none of its instructions."),
            "{}",
            codex_note(&v)
        );
        write(
            &r.ext.join(".codex").join("config.toml"),
            &format!("[projects.'{key}']\ntrust_level = \"trusted\"\n"),
        );
        let v = view_ok(&r.st, Some(&r.project)).await;
        assert!(!codex_note(&v).contains("trust"), "{}", codex_note(&v));
    }

    #[tokio::test]
    async fn save_writes_a_project_doc_and_renames_it() {
        let r = rig();
        let project = r.project.to_string_lossy().into_owned();
        let mut req = save_of(
            "api",
            "Use the API.
",
        );
        req.scope = marshell_protocol::instructions::DocScope::Project;
        req.project = Some(project.clone());
        let (status, body) = post(&r.st, "save", &req).await;
        assert_eq!(status, StatusCode::OK, "{body}");
        let file = r.project.join(".marshell").join("instructions").join("api.md");
        assert!(std::fs::read_to_string(&file).unwrap().contains("enabled: true"));
        let saved: DocSaved = serde_json::from_str(&body).unwrap();
        assert_eq!(saved.view.project_docs.len(), 1);
        // A rename writes the new file and removes the old one; a name that differs by case only is refused.
        let hash = saved.view.project_docs[0].doc.hash.clone();
        let mut rename = req.clone();
        rename.old_name = Some("api".into());
        rename.expected_hash = Some(hash);
        rename.name = "API".into();
        let (status, body) = post(&r.st, "save", &rename).await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(body, "A new name must differ by more than case.");
        rename.name = "rest-api".into();
        let (status, body) = post(&r.st, "save", &rename).await;
        assert_eq!(status, StatusCode::OK, "{body}");
        assert!(!file.exists());
        assert!(file.with_file_name("rest-api.md").exists());
        // A project doc without its project is refused.
        let mut lost = req;
        lost.project = None;
        lost.name = "other".into();
        assert_eq!(post(&r.st, "save", &lost).await.0, StatusCode::UNPROCESSABLE_ENTITY);
    }

    #[tokio::test]
    async fn routes_refuse_without_the_bearer() {
        let r = rig();
        for (method, uri) in [
            ("GET", "/v1/instructions"),
            ("POST", "/v1/instructions/save"),
            ("POST", "/v1/instructions/review"),
            ("POST", "/v1/instructions/delete"),
            ("POST", "/v1/instructions/copy"),
        ] {
            let (status, _) = call(&r.st, method, uri, Some(serde_json::json!({})), false).await;
            assert_eq!(status, StatusCode::UNAUTHORIZED, "{method} {uri}");
        }
    }
}
