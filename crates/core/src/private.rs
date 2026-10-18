//! Makes the Marshell home owner-only, so the endpoint token, recordings and attachments under it are
//! unreadable to other local accounts. Run once on every start, before anything is written there.

use std::path::Path;

/// What `secure_home` found.
#[derive(Debug, PartialEq, Eq)]
pub enum HomeAcl {
    /// The home already had the owner-only permissions; nothing was touched.
    AlreadyPrivate,
    /// The home (and everything under it) was changed to owner-only.
    Fixed,
}

/// Creates the home if needed and makes it owner-only.
///
/// Windows: a protected DACL with two allow ACEs, the current user and SYSTEM (S-1-5-18), full control,
/// inherited by files and folders (`CONTAINER_INHERIT_ACE | OBJECT_INHERIT_ACE`). When the home's DACL is
/// not exactly that, it is set with `TreeResetNamedSecurityInfoW` (KeepExplicit = FALSE), so every child
/// goes back to inheriting it.
///
/// Unix: folders 0700, files 0600, a file with the owner's x bit 0700. Links are not followed.
///
/// Only the home's own permissions are checked, on both OSes, so a launch costs one call and never a walk of
/// the recordings and attachments. Children are fixed only when the home itself is not private. On Unix that
/// means the home's mode is not 0700; a child someone loosened later is not noticed until then.
pub fn secure_home(home: &Path) -> anyhow::Result<HomeAcl> {
    std::fs::create_dir_all(home)?;
    #[cfg(windows)]
    return win::secure(home);
    #[cfg(unix)]
    return unix::secure(home);
    #[cfg(not(any(windows, unix)))]
    return Ok(HomeAcl::AlreadyPrivate);
}

#[cfg(windows)]
pub use win::{Dacl, current_user_sid, dacl_of};

#[cfg(unix)]
mod unix {
    use super::HomeAcl;
    use std::fs::{self, Permissions};
    use std::os::unix::fs::PermissionsExt;
    use std::path::{Path, PathBuf};

    pub fn secure(home: &Path) -> anyhow::Result<HomeAcl> {
        // The home itself may be a link to a folder the user chose; its target is what gets checked.
        if fs::metadata(home)?.permissions().mode() & 0o777 == 0o700 {
            return Ok(HomeAcl::AlreadyPrivate);
        }
        let mut pending: Vec<PathBuf> = vec![home.to_path_buf()];
        while let Some(dir) = pending.pop() {
            set_mode(&dir, 0o700)?;
            for entry in fs::read_dir(&dir)? {
                let path = entry?.path();
                // symlink_metadata: a link is skipped, never followed, so nothing outside the home changes.
                let meta = fs::symlink_metadata(&path)?;
                if meta.is_dir() {
                    pending.push(path);
                } else if meta.is_file() {
                    let want = if meta.permissions().mode() & 0o100 != 0 {
                        0o700
                    } else {
                        0o600
                    };
                    set_mode(&path, want)?;
                }
            }
        }
        Ok(HomeAcl::Fixed)
    }

    fn set_mode(path: &Path, mode: u32) -> anyhow::Result<()> {
        let current = fs::symlink_metadata(path)?.permissions().mode() & 0o777;
        if current != mode {
            fs::set_permissions(path, Permissions::from_mode(mode))
                .map_err(|e| anyhow::anyhow!("could not set mode {mode:o} on {}: {e}", path.display()))?;
        }
        Ok(())
    }
}

#[cfg(windows)]
mod win {
    use super::HomeAcl;
    use anyhow::{Context, bail};
    use std::ffi::c_void;
    use std::os::windows::ffi::OsStrExt;
    use std::path::Path;
    use std::ptr::{null, null_mut};
    use windows_sys::Win32::Foundation::{CloseHandle, ERROR_SUCCESS, HANDLE, LocalFree};
    use windows_sys::Win32::Security::Authorization::{
        ConvertSidToStringSidW, GetNamedSecurityInfoW, ProgressInvokeNever, SE_FILE_OBJECT, TreeResetNamedSecurityInfoW,
    };
    use windows_sys::Win32::Security::{
        ACCESS_ALLOWED_ACE, ACE_HEADER, ACL, ACL_REVISION, AddAccessAllowedAceEx, CONTAINER_INHERIT_ACE,
        CreateWellKnownSid, DACL_SECURITY_INFORMATION, GetAce, GetLengthSid, GetSecurityDescriptorControl,
        GetTokenInformation, INHERITED_ACE, InitializeAcl, OBJECT_INHERIT_ACE, PROTECTED_DACL_SECURITY_INFORMATION,
        PSID, SE_DACL_PROTECTED, SECURITY_MAX_SID_SIZE, TOKEN_QUERY, TOKEN_USER, TokenUser, WinLocalSystemSid,
    };
    use windows_sys::Win32::Storage::FileSystem::FILE_ALL_ACCESS;
    use windows_sys::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};

    /// ACE type of an allow ACE (`ACCESS_ALLOWED_ACE_TYPE`, which lives in a feature this crate does not use).
    const ACCESS_ALLOWED_ACE_TYPE: u8 = 0;
    const INHERIT: u32 = CONTAINER_INHERIT_ACE | OBJECT_INHERIT_ACE;

    /// A path's DACL as far as the owner-only check needs it.
    pub struct Dacl {
        /// The DACL does not inherit from the parent (`SE_DACL_PROTECTED`).
        pub protected: bool,
        /// String SIDs of the `ACCESS_ALLOWED` ACEs, in ACE order.
        pub allow_sids: Vec<String>,
        /// Every ACE carries `INHERITED_ACE`.
        pub all_inherited: bool,
    }

    /// One ACE, reduced to what the exact-match check compares.
    struct Ace {
        ace_type: u8,
        flags: u8,
        /// Meaningful only for `ACCESS_ALLOWED_ACE_TYPE`.
        mask: u32,
        /// Meaningful only for `ACCESS_ALLOWED_ACE_TYPE`.
        sid: String,
    }

    struct Read {
        protected: bool,
        /// False for a NULL DACL, which grants everyone full access.
        has_dacl: bool,
        aces: Vec<Ace>,
    }

    pub fn dacl_of(path: &Path) -> anyhow::Result<Dacl> {
        let read = read_dacl(path)?;
        Ok(Dacl {
            protected: read.protected,
            allow_sids: read
                .aces
                .iter()
                .filter(|a| a.ace_type == ACCESS_ALLOWED_ACE_TYPE)
                .map(|a| a.sid.clone())
                .collect(),
            all_inherited: read.aces.iter().all(|a| u32::from(a.flags) & INHERITED_ACE != 0),
        })
    }

    pub fn current_user_sid() -> anyhow::Result<String> {
        let buf = token_user()?;
        // SAFETY: `token_user` filled the 8-byte-aligned `buf` with a TOKEN_USER whose SID pointer points into the same buffer.
        let sid = unsafe { (*buf.as_ptr().cast::<TOKEN_USER>()).User.Sid };
        sid_to_string(sid)
    }

    pub fn secure(home: &Path) -> anyhow::Result<HomeAcl> {
        let user = sid_bytes_of_user()?;
        let system = well_known_system_sid()?;
        let mut sids: Vec<(String, &[u32])> = vec![(sid_to_string(user.as_ptr() as PSID)?, user.as_slice())];
        let system_string = sid_to_string(system.as_ptr() as PSID)?;
        // Running as SYSTEM: the user and SYSTEM are one SID, and one ACE.
        if sids[0].0 != system_string {
            sids.push((system_string, system.as_slice()));
        }

        if is_exactly(
            &read_dacl(home)?,
            &sids.iter().map(|(s, _)| s.as_str()).collect::<Vec<_>>(),
        ) {
            return Ok(HomeAcl::AlreadyPrivate);
        }

        let acl = build_acl(&sids.iter().map(|(_, b)| *b).collect::<Vec<_>>())?;
        let wide = wide_path(home);
        // SAFETY: `wide` is NUL-terminated and `acl` is a valid ACL that outlive the call; owner, group, SACL
        // and the callback are unused. KeepExplicit = 0 clears every child's own ACEs so each inherits.
        let err = unsafe {
            TreeResetNamedSecurityInfoW(
                wide.as_ptr(),
                SE_FILE_OBJECT,
                DACL_SECURITY_INFORMATION | PROTECTED_DACL_SECURITY_INFORMATION,
                null_mut(),
                null_mut(),
                acl.as_ptr().cast::<ACL>(),
                null(),
                0,
                None,
                ProgressInvokeNever,
                null(),
            )
        };
        if err != ERROR_SUCCESS {
            bail!("could not make {} owner-only: Win32 error {err}", home.display());
        }
        Ok(HomeAcl::Fixed)
    }

    /// True when the DACL is protected and holds exactly one allow ACE per wanted SID, each with full
    /// control and the inherit flags, and nothing else (no deny, no inherited, no object or callback ACE).
    fn is_exactly(read: &Read, wanted: &[&str]) -> bool {
        if !read.protected || !read.has_dacl || read.aces.len() != wanted.len() {
            return false;
        }
        let mut seen: Vec<&str> = Vec::new();
        for ace in &read.aces {
            if ace.ace_type != ACCESS_ALLOWED_ACE_TYPE || u32::from(ace.flags) != INHERIT || ace.mask != FILE_ALL_ACCESS
            {
                return false;
            }
            seen.push(&ace.sid);
        }
        wanted.iter().all(|w| seen.contains(w))
    }

    /// Owns memory the system allocated with LocalAlloc and frees it when dropped.
    struct LocalMem(*mut c_void);

    impl Drop for LocalMem {
        fn drop(&mut self) {
            if !self.0.is_null() {
                // SAFETY: the pointer came from an API documented to return LocalAlloc memory, freed once.
                unsafe { LocalFree(self.0) };
            }
        }
    }

    fn wide_path(path: &Path) -> Vec<u16> {
        path.as_os_str().encode_wide().chain(Some(0)).collect()
    }

    fn read_dacl(path: &Path) -> anyhow::Result<Read> {
        let wide = wide_path(path);
        let mut dacl: *mut ACL = null_mut();
        let mut sd: *mut c_void = null_mut();
        // SAFETY: `wide` is NUL-terminated; only the DACL is requested, so the other out-pointers may be null.
        // `dacl` points into the descriptor `sd`, which `_sd` frees after the last use below.
        let err = unsafe {
            GetNamedSecurityInfoW(
                wide.as_ptr(),
                SE_FILE_OBJECT,
                DACL_SECURITY_INFORMATION,
                null_mut(),
                null_mut(),
                &mut dacl,
                null_mut(),
                &mut sd,
            )
        };
        if err != ERROR_SUCCESS {
            bail!(
                "could not read the permissions of {}: Win32 error {err}",
                path.display()
            );
        }
        let _sd = LocalMem(sd);

        let mut control: u16 = 0;
        let mut revision: u32 = 0;
        // SAFETY: `sd` is the valid descriptor returned above; both out-pointers are live locals.
        if unsafe { GetSecurityDescriptorControl(sd, &mut control, &mut revision) } == 0 {
            bail!(
                "could not read the control flags of {}: {}",
                path.display(),
                std::io::Error::last_os_error()
            );
        }
        let protected = control & SE_DACL_PROTECTED != 0;
        if dacl.is_null() {
            return Ok(Read {
                protected,
                has_dacl: false,
                aces: Vec::new(),
            });
        }

        // SAFETY: `dacl` is non-null and valid while `_sd` lives.
        let count = unsafe { (*dacl).AceCount };
        let mut aces = Vec::with_capacity(count.into());
        for index in 0..u32::from(count) {
            let mut ace: *mut c_void = null_mut();
            // SAFETY: `dacl` is valid and `index` is below its AceCount; `ace` receives a pointer into it.
            if unsafe { GetAce(dacl, index, &mut ace) } == 0 {
                bail!(
                    "could not read ACE {index} of {}: {}",
                    path.display(),
                    std::io::Error::last_os_error()
                );
            }
            // SAFETY: every ACE starts with an ACE_HEADER.
            let header = unsafe { &*ace.cast::<ACE_HEADER>() };
            let (mask, sid) = if header.AceType == ACCESS_ALLOWED_ACE_TYPE {
                let allowed = ace.cast::<ACCESS_ALLOWED_ACE>();
                // SAFETY: the type says this is an ACCESS_ALLOWED_ACE; its SID starts at `SidStart`.
                let (mask, sid_ptr) = unsafe { ((*allowed).Mask, (&raw mut (*allowed).SidStart).cast::<c_void>()) };
                (mask, sid_to_string(sid_ptr)?)
            } else {
                (0, String::new())
            };
            aces.push(Ace {
                ace_type: header.AceType,
                flags: header.AceFlags,
                mask,
                sid,
            });
        }
        Ok(Read {
            protected,
            has_dacl: true,
            aces,
        })
    }

    fn sid_to_string(sid: PSID) -> anyhow::Result<String> {
        let mut out: *mut u16 = null_mut();
        // SAFETY: `sid` is a valid SID; `out` receives a LocalAlloc'd NUL-terminated string.
        if unsafe { ConvertSidToStringSidW(sid, &mut out) } == 0 {
            bail!("could not format a SID: {}", std::io::Error::last_os_error());
        }
        let _free = LocalMem(out.cast());
        // SAFETY: `out` is NUL-terminated, so scanning to the terminator stays inside the allocation.
        let len = (0..).take_while(|&i| unsafe { *out.add(i) } != 0).count();
        // SAFETY: the `len` units before the terminator are initialized.
        Ok(String::from_utf16_lossy(unsafe {
            std::slice::from_raw_parts(out, len)
        }))
    }

    /// The TOKEN_USER of this process, in a u64 buffer: the structure holds a pointer, so it needs
    /// the buffer aligned to 8 bytes on 64-bit Windows, which a `Vec<u32>` does not promise.
    fn token_user() -> anyhow::Result<Vec<u64>> {
        let mut token: HANDLE = null_mut();
        // SAFETY: GetCurrentProcess is a pseudo-handle; `token` receives a handle closed below.
        if unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) } == 0 {
            bail!("could not open the process token: {}", std::io::Error::last_os_error());
        }
        let result = (|| {
            let mut needed = 0u32;
            // SAFETY: a null buffer of length 0 asks for the size; `needed` is a live local.
            unsafe { GetTokenInformation(token, TokenUser, null_mut(), 0, &mut needed) };
            if needed == 0 {
                bail!("could not size the token user: {}", std::io::Error::last_os_error());
            }
            let mut buf = vec![0u64; (needed as usize).div_ceil(8)];
            // SAFETY: the buffer holds at least `needed` bytes and is 8-byte aligned, which TOKEN_USER needs.
            if unsafe { GetTokenInformation(token, TokenUser, buf.as_mut_ptr().cast(), needed, &mut needed) } == 0 {
                bail!("could not read the token user: {}", std::io::Error::last_os_error());
            }
            Ok(buf)
        })();
        // SAFETY: `token` is a valid handle opened above and not used afterwards.
        unsafe { CloseHandle(token) };
        result
    }

    /// A copy of the current user's SID, in a u32 buffer.
    fn sid_bytes_of_user() -> anyhow::Result<Vec<u32>> {
        let buf = token_user()?;
        // SAFETY: `token_user` filled the 8-byte-aligned `buf` with a TOKEN_USER whose SID pointer points into the same buffer.
        let sid = unsafe { (*buf.as_ptr().cast::<TOKEN_USER>()).User.Sid };
        // SAFETY: `sid` is a valid SID.
        let len = unsafe { GetLengthSid(sid) } as usize;
        // SAFETY: `len` bytes at `sid` belong to `buf`, which is still alive.
        let bytes = unsafe { std::slice::from_raw_parts(sid.cast::<u8>(), len) };
        Ok(copy_to_words(bytes))
    }

    fn well_known_system_sid() -> anyhow::Result<Vec<u32>> {
        let mut buf = vec![0u32; (SECURITY_MAX_SID_SIZE as usize).div_ceil(4)];
        let mut size = SECURITY_MAX_SID_SIZE;
        // SAFETY: `buf` holds `size` bytes; a null domain SID is allowed for a machine-wide well-known SID.
        if unsafe { CreateWellKnownSid(WinLocalSystemSid, null_mut(), buf.as_mut_ptr().cast(), &mut size) } == 0 {
            bail!("could not build the SYSTEM SID: {}", std::io::Error::last_os_error());
        }
        Ok(buf)
    }

    fn copy_to_words(bytes: &[u8]) -> Vec<u32> {
        let mut words = vec![0u32; bytes.len().div_ceil(4)];
        // SAFETY: `words` has room for `bytes.len()` bytes and the two do not overlap.
        unsafe { std::ptr::copy_nonoverlapping(bytes.as_ptr(), words.as_mut_ptr().cast::<u8>(), bytes.len()) };
        words
    }

    /// An ACL with one full-control, inheritable allow ACE per SID, in a u32 buffer (ACLs are DWORD-aligned).
    fn build_acl(sids: &[&[u32]]) -> anyhow::Result<Vec<u32>> {
        let mut size = size_of::<ACL>();
        for sid in sids {
            // SAFETY: each buffer holds a valid SID.
            let len = unsafe { GetLengthSid(sid.as_ptr() as PSID) } as usize;
            // ACCESS_ALLOWED_ACE already counts 4 bytes of SID, so this is a little more than needed.
            size += size_of::<ACCESS_ALLOWED_ACE>() + len;
        }
        let size = size.next_multiple_of(4);
        let mut acl = vec![0u32; size / 4];
        // SAFETY: the buffer is `size` bytes and DWORD-aligned.
        if unsafe { InitializeAcl(acl.as_mut_ptr().cast(), size as u32, ACL_REVISION) } == 0 {
            bail!("could not build an ACL: {}", std::io::Error::last_os_error());
        }
        for sid in sids {
            // SAFETY: the ACL was initialized with room for every ACE; each buffer holds a valid SID.
            let added = unsafe {
                AddAccessAllowedAceEx(
                    acl.as_mut_ptr().cast(),
                    ACL_REVISION,
                    INHERIT,
                    FILE_ALL_ACCESS,
                    sid.as_ptr() as PSID,
                )
            };
            if added == 0 {
                return Err(std::io::Error::last_os_error()).context("could not add an ACE");
            }
        }
        Ok(acl)
    }
}
