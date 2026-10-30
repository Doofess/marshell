//! `marshell new [options]`: starts a session in the running app, and with `--prompt` puts text on its prompt line
//! once the agent is ready. Marshell never presses Enter, so the person does that themselves.
//!
//! Exit codes: 0 started (and filled), 1 refused, failed, or the app is closed and wasn't started.

use crate::start::ensure_running;
use marshell_core::server::check_text;
use marshell_protocol::api::{CreateSessionRequest, SessionMode};
use std::io::{BufRead, Write};
use std::path::Path;

/// What `new` was asked for, as `args` parsed it.
pub struct Options {
    pub agent: Option<String>,
    /// Absolute. `None` is the CLI's own folder.
    pub cwd: Option<String>,
    pub prompt: Option<String>,
    pub mode: Option<SessionMode>,
    pub name: Option<String>,
    pub start: bool,
}

/// The size the session starts at: the window resizes it when it attaches.
const COLS: u16 = 120;
const ROWS: u16 = 30;

/// The request that creates the session. `prompt` is always `None`: the core's first-prompt argument starts the
/// agent with the text as an argument, which `--prompt` must not do. The text goes in through `fill`, once the agent
/// is ready, as a line the person sends.
pub fn create_request(
    agent: Option<String>,
    cwd: String,
    name: Option<String>,
    mode: Option<SessionMode>,
) -> CreateSessionRequest {
    CreateSessionRequest {
        shell: None,
        cwd: Some(cwd),
        cols: COLS,
        rows: ROWS,
        profile_id: None,
        agent,
        name,
        prompt: None,
        mode,
        setup: None,
        preset: None,
        color_scheme: None,
        tool: None,
        cli_task: None,
    }
}

pub fn run(
    env: &dyn Fn(&str) -> Option<String>,
    options: Options,
    current_dir: &Path,
    stdin_is_tty: bool,
    ask: &mut dyn BufRead,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> i32 {
    // The text is checked before anything starts: a refused prompt creates no session and asks no question.
    if let Some(prompt) = options.prompt.as_deref()
        && let Err(refusal) = check_text(prompt)
    {
        let _ = writeln!(err, "{refusal}");
        return 1;
    }
    let client = match ensure_running(env, stdin_is_tty, ask, out, options.start) {
        Ok(client) => client,
        Err(line) => {
            let _ = writeln!(err, "{line}");
            return 1;
        }
    };
    let cwd = options
        .cwd
        .unwrap_or_else(|| current_dir.to_string_lossy().into_owned());
    let created = match client.create(&create_request(options.agent, cwd, options.name, options.mode)) {
        Ok(created) => created,
        Err(line) => {
            let _ = writeln!(err, "{line}");
            return 1;
        }
    };
    let number = created.number;
    let _ = writeln!(out, "Started session {number}.");
    let Some(prompt) = options.prompt else {
        return 0;
    };
    // The core holds the text back until the agent is ready, up to 30 s. A session that wasn't ready stays open.
    match client.fill(&created.tab_id, &prompt, true) {
        Ok(()) => {
            let _ = writeln!(out, "Filled session {number}'s prompt line.");
            0
        }
        Err(refusal) => {
            let _ = writeln!(err, "{}", refusal.message(number));
            1
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_request_never_carries_the_prompt() {
        let req = create_request(
            Some("claude".into()),
            "C:\\work".into(),
            Some("n".into()),
            Some(SessionMode::Plan),
        );
        assert!(req.prompt.is_none());
        assert_eq!((req.cols, req.rows), (120, 30));
        assert_eq!(req.agent.as_deref(), Some("claude"));
        assert_eq!(req.cwd.as_deref(), Some("C:\\work"));
        assert_eq!(req.name.as_deref(), Some("n"));
        assert_eq!(req.mode, Some(SessionMode::Plan));
        assert!(req.shell.is_none() && req.profile_id.is_none() && req.preset.is_none());
        // On the wire: no `prompt` key at all.
        let body = serde_json::to_value(&req).unwrap();
        assert!(body.get("prompt").is_none(), "{body}");
        // A plain shell: no agent, no mode.
        let plain = serde_json::to_value(create_request(None, "C:\\work".into(), None, None)).unwrap();
        assert!(plain.get("agent").is_none() && plain.get("mode").is_none(), "{plain}");
    }

    #[test]
    fn a_refused_prompt_stops_before_the_app_is_asked() {
        let home = tempfile::tempdir().unwrap();
        let path = home.path().to_string_lossy().into_owned();
        let env = move |k: &str| (k == "MARSHELL_HOME").then(|| path.clone());
        let (mut out, mut err) = (Vec::new(), Vec::new());
        // A terminal that would be asked if the app were: the refusal comes first, with no question.
        let options = Options {
            agent: None,
            cwd: None,
            prompt: Some("a\nb".into()),
            mode: None,
            name: None,
            start: false,
        };
        let code = run(
            &env,
            options,
            home.path(),
            true,
            &mut std::io::empty(),
            &mut out,
            &mut err,
        );
        assert_eq!(code, 1);
        assert_eq!(out, b"");
        assert_eq!(
            String::from_utf8(err).unwrap().trim_end(),
            "The text can't hold line breaks or other control characters: Marshell never presses Enter."
        );
    }
}
