//! Every user-visible name lives here, so a rename is one edit.
//! The frontend gets these through `src/generated/constants.ts`.

pub const APP_NAME: &str = "Marshell";
pub const CLI_BIN: &str = "marshell";
pub const HOOK_BIN: &str = "marshell-hook";
pub const HOME_DIR_NAME: &str = ".marshell";
pub const ENV_PREFIX: &str = "MARSHELL_";
pub const WS_SUBPROTOCOL: &str = "marshell.v1";
pub const TOKEN_SUBPROTOCOL_PREFIX: &str = "token.";

/// `env_var("TAB_ID")` -> `"MARSHELL_TAB_ID"`.
pub fn env_var(name: &str) -> String {
    format!("{ENV_PREFIX}{name}")
}
