//! A Job Object that kills every process in it when it is closed or terminated, so a hung status
//! line takes its grandchildren with it. On other OSes it does nothing and the caller kills the child.

#[cfg(windows)]
pub use windows::{Job, resume};

#[cfg(not(windows))]
pub use other::{Job, resume};

#[cfg(windows)]
mod windows {
    use std::io;
    use std::os::windows::io::AsRawHandle;
    use std::process::Child;
    use windows_sys::Win32::Foundation::{CloseHandle, HANDLE, INVALID_HANDLE_VALUE};
    use windows_sys::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, TH32CS_SNAPTHREAD, THREADENTRY32, Thread32First, Thread32Next,
    };
    use windows_sys::Win32::System::JobObjects::{
        AssignProcessToJobObject, CreateJobObjectW, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
        JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JobObjectExtendedLimitInformation, SetInformationJobObject,
        TerminateJobObject,
    };
    use windows_sys::Win32::System::Threading::{OpenThread, ResumeThread, THREAD_SUSPEND_RESUME};

    pub struct Job(HANDLE);

    impl Job {
        pub fn new() -> io::Result<Job> {
            // SAFETY: both arguments may be null (default security, no name); the result is checked.
            let handle = unsafe { CreateJobObjectW(std::ptr::null(), std::ptr::null()) };
            if handle.is_null() {
                return Err(io::Error::last_os_error());
            }
            // From here `job` owns the handle, so an early return closes it.
            let job = Job(handle);
            // SAFETY: an all-zero JOBOBJECT_EXTENDED_LIMIT_INFORMATION is valid (plain integers and sizes).
            let mut info: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = unsafe { std::mem::zeroed() };
            info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
            // SAFETY: `handle` is valid; the pointer and size describe `info`, which outlives the call.
            let set = unsafe {
                SetInformationJobObject(
                    handle,
                    JobObjectExtendedLimitInformation,
                    (&raw const info).cast(),
                    size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
                )
            };
            if set == 0 {
                return Err(io::Error::last_os_error());
            }
            Ok(job)
        }

        /// Puts `child` in the job. Its own children join the job when they start.
        pub fn assign(&self, child: &Child) -> io::Result<()> {
            // SAFETY: both handles are valid; `child` owns its process handle for the whole call.
            if unsafe { AssignProcessToJobObject(self.0, child.as_raw_handle()) } == 0 {
                return Err(io::Error::last_os_error());
            }
            Ok(())
        }

        /// Ends every process in the job.
        pub fn terminate(&self) {
            // SAFETY: `self.0` is a valid job handle. A failure leaves the processes running, and the
            // caller still kills the child itself.
            unsafe { TerminateJobObject(self.0, 1) };
        }
    }

    impl Drop for Job {
        fn drop(&mut self) {
            // SAFETY: `self.0` is a valid handle that nothing else uses. Closing it kills what is left in the job.
            unsafe { CloseHandle(self.0) };
        }
    }

    /// Resumes a child that was started with `CREATE_SUSPENDED`. `std` keeps the main thread's handle
    /// to itself, so the thread is found by its owner: a suspended process has only the one.
    pub fn resume(child: &Child) -> io::Result<()> {
        let pid = child.id();
        // SAFETY: plain call; the result is checked.
        let snapshot = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPTHREAD, 0) };
        if snapshot == INVALID_HANDLE_VALUE {
            return Err(io::Error::last_os_error());
        }
        let result = resume_thread_of(snapshot, pid);
        // SAFETY: `snapshot` is a valid handle that nothing else uses.
        unsafe { CloseHandle(snapshot) };
        result
    }

    fn resume_thread_of(snapshot: HANDLE, pid: u32) -> io::Result<()> {
        // SAFETY: an all-zero THREADENTRY32 is valid (plain integers); dwSize is set before use.
        let mut entry: THREADENTRY32 = unsafe { std::mem::zeroed() };
        entry.dwSize = size_of::<THREADENTRY32>() as u32;
        // SAFETY: `snapshot` is valid and `entry` is sized as dwSize says.
        let mut more = unsafe { Thread32First(snapshot, &mut entry) } != 0;
        while more {
            if entry.th32OwnerProcessID == pid {
                // SAFETY: plain call; the result is checked.
                let thread = unsafe { OpenThread(THREAD_SUSPEND_RESUME, 0, entry.th32ThreadID) };
                if thread.is_null() {
                    return Err(io::Error::last_os_error());
                }
                // SAFETY: `thread` is a valid handle with the right to resume.
                let resumed = unsafe { ResumeThread(thread) };
                let err = io::Error::last_os_error();
                // SAFETY: `thread` is a valid handle that nothing else uses.
                unsafe { CloseHandle(thread) };
                return if resumed == u32::MAX { Err(err) } else { Ok(()) };
            }
            // SAFETY: as for Thread32First.
            more = unsafe { Thread32Next(snapshot, &mut entry) } != 0;
        }
        Err(io::Error::other("the suspended process has no thread to resume"))
    }
}

#[cfg(not(windows))]
mod other {
    use std::io;
    use std::process::Child;

    pub struct Job;

    impl Job {
        pub fn new() -> io::Result<Job> {
            Ok(Job)
        }

        pub fn assign(&self, _child: &Child) -> io::Result<()> {
            Ok(())
        }

        pub fn terminate(&self) {}
    }

    pub fn resume(_child: &Child) -> io::Result<()> {
        Ok(())
    }
}
