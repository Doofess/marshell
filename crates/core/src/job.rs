//! On Windows, puts Marshell in a Job Object that kills every process in it when Marshell ends,
//! however it ends (a crash or Task Manager included). Children join the job when they start,
//! so no shell, agent or OpenConsole is left behind. A program started with
//! `CREATE_BREAKAWAY_FROM_JOB` can still outlive Marshell on purpose.

/// Call once, first thing in `main`. A no-op on other OSes.
pub fn tie_children_to_app() -> anyhow::Result<()> {
    #[cfg(windows)]
    windows::tie()?;
    Ok(())
}

#[cfg(windows)]
mod windows {
    use windows_sys::Win32::Foundation::CloseHandle;
    use windows_sys::Win32::System::JobObjects::{
        AssignProcessToJobObject, CreateJobObjectW, JOB_OBJECT_LIMIT_BREAKAWAY_OK, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
        JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JobObjectExtendedLimitInformation, SetInformationJobObject,
    };
    use windows_sys::Win32::System::Threading::GetCurrentProcess;

    pub fn tie() -> anyhow::Result<()> {
        // SAFETY: both arguments may be null (default security, no name); the result is checked.
        let job = unsafe { CreateJobObjectW(std::ptr::null(), std::ptr::null()) };
        if job.is_null() {
            anyhow::bail!("could not create a job object: {}", std::io::Error::last_os_error());
        }

        // SAFETY: an all-zero JOBOBJECT_EXTENDED_LIMIT_INFORMATION is valid (plain integers and sizes).
        let mut info: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = unsafe { std::mem::zeroed() };
        info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE | JOB_OBJECT_LIMIT_BREAKAWAY_OK;
        // SAFETY: `job` is a valid handle; the pointer and size describe `info`, which outlives the call.
        let set = unsafe {
            SetInformationJobObject(
                job,
                JobObjectExtendedLimitInformation,
                (&raw const info).cast(),
                size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
            )
        };
        // SAFETY: GetCurrentProcess returns a pseudo-handle that needs no closing; `job` is valid.
        let assigned = set != 0 && unsafe { AssignProcessToJobObject(job, GetCurrentProcess()) } != 0;
        if !assigned {
            let err = std::io::Error::last_os_error();
            // SAFETY: `job` is a valid handle that nothing else uses.
            unsafe { CloseHandle(job) };
            anyhow::bail!("could not put Marshell in a job object: {err}");
        }
        // The handle is never closed on purpose: Windows closes it when Marshell ends, and that kills the job.
        Ok(())
    }
}
