//! On Windows, puts Marshell in a Job Object that kills every process in it when Marshell crashes
//! or is killed (Task Manager included). Children join the job when they start, so no shell, agent
//! or OpenConsole is left behind. On a normal exit `release_children` switches that off: sessions
//! end on their own, and programs the user started from a shell (VS Code, a browser) keep running.
//! A program started with `CREATE_BREAKAWAY_FROM_JOB` is never in the job.

/// Call once, first thing in `main`. A no-op on other OSes.
pub fn tie_children_to_app() -> anyhow::Result<()> {
    #[cfg(windows)]
    windows::tie()?;
    Ok(())
}

/// Call on a normal exit, after the sessions have ended, so closing Marshell does not kill
/// programs the user started from a shell. Does nothing if `tie_children_to_app` failed.
pub fn release_children() {
    #[cfg(windows)]
    windows::release();
}

#[cfg(windows)]
mod windows {
    use std::sync::OnceLock;
    use windows_sys::Win32::Foundation::{CloseHandle, HANDLE};
    use windows_sys::Win32::System::JobObjects::{
        AssignProcessToJobObject, CreateJobObjectW, JOB_OBJECT_LIMIT_BREAKAWAY_OK, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
        JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JobObjectExtendedLimitInformation, SetInformationJobObject,
    };
    use windows_sys::Win32::System::Threading::GetCurrentProcess;

    /// The job's handle, kept as a number because a raw handle cannot be stored in a static.
    static JOB: OnceLock<usize> = OnceLock::new();

    pub fn tie() -> anyhow::Result<()> {
        // SAFETY: both arguments may be null (default security, no name); the result is checked.
        let job = unsafe { CreateJobObjectW(std::ptr::null(), std::ptr::null()) };
        if job.is_null() {
            anyhow::bail!("could not create a job object: {}", std::io::Error::last_os_error());
        }
        let limited = set_limits(job, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE | JOB_OBJECT_LIMIT_BREAKAWAY_OK);
        // SAFETY: GetCurrentProcess returns a pseudo-handle that needs no closing; `job` is valid.
        let assigned = limited && unsafe { AssignProcessToJobObject(job, GetCurrentProcess()) } != 0;
        if !assigned {
            let err = std::io::Error::last_os_error();
            // SAFETY: `job` is a valid handle that nothing else uses.
            unsafe { CloseHandle(job) };
            anyhow::bail!("could not put Marshell in a job object: {err}");
        }
        // The handle is never closed on purpose: Windows closes it when Marshell ends, and that kills the job.
        let _ = JOB.set(job as usize);
        Ok(())
    }

    pub fn release() {
        if let Some(&job) = JOB.get()
            && !set_limits(job as HANDLE, JOB_OBJECT_LIMIT_BREAKAWAY_OK)
        {
            eprintln!(
                "Marshell: could not release child processes: {}",
                std::io::Error::last_os_error()
            );
        }
    }

    /// Replaces the job's limit flags. Returns false on failure (see `last_os_error`).
    fn set_limits(job: HANDLE, flags: u32) -> bool {
        // SAFETY: an all-zero JOBOBJECT_EXTENDED_LIMIT_INFORMATION is valid (plain integers and sizes).
        let mut info: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = unsafe { std::mem::zeroed() };
        info.BasicLimitInformation.LimitFlags = flags;
        // SAFETY: `job` is a valid handle; the pointer and size describe `info`, which outlives the call.
        let set = unsafe {
            SetInformationJobObject(
                job,
                JobObjectExtendedLimitInformation,
                (&raw const info).cast(),
                size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
            )
        };
        set != 0
    }
}
