//! Whether this bridge runs as administrator: an agent in an elevated Marshell session runs its hooks
//! elevated, and such a bridge trusts less of what the user-writable `endpoint.json` says (slice 6j2
//! decision 12).

/// True when this process's token is elevated.
#[cfg(windows)]
pub fn is_elevated() -> bool {
    use windows_sys::Win32::Foundation::{CloseHandle, HANDLE};
    use windows_sys::Win32::Security::{GetTokenInformation, TOKEN_ELEVATION, TOKEN_QUERY, TokenElevation};
    use windows_sys::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};

    // SAFETY: the token handle is checked before use and closed afterwards; the out-pointers describe
    // locals that outlive the calls.
    unsafe {
        let mut token: HANDLE = std::ptr::null_mut();
        if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) == 0 {
            return false;
        }
        let mut elevation = TOKEN_ELEVATION { TokenIsElevated: 0 };
        let mut len = 0u32;
        let ok = GetTokenInformation(
            token,
            TokenElevation,
            (&raw mut elevation).cast(),
            size_of::<TOKEN_ELEVATION>() as u32,
            &mut len,
        );
        CloseHandle(token);
        ok != 0 && elevation.TokenIsElevated != 0
    }
}

/// Marshell starts administrator sessions on Windows only (slice 6j2 decision 4).
#[cfg(not(windows))]
pub fn is_elevated() -> bool {
    false
}
