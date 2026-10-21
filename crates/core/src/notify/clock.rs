//! The local time of day, for quiet hours.

/// Minutes since local midnight, 0..1440.
#[cfg(windows)]
pub fn local_minute_of_day() -> u16 {
    use windows_sys::Win32::Foundation::SYSTEMTIME;
    use windows_sys::Win32::System::SystemInformation::GetLocalTime;

    let mut now = SYSTEMTIME {
        wYear: 0,
        wMonth: 0,
        wDayOfWeek: 0,
        wDay: 0,
        wHour: 0,
        wMinute: 0,
        wSecond: 0,
        wMilliseconds: 0,
    };
    // SAFETY: `now` is a valid, writable SYSTEMTIME, and the call has no other input.
    unsafe { GetLocalTime(&mut now) };
    (now.wHour * 60 + now.wMinute).min(1439)
}

/// Minutes since local midnight, 0..1440. 0 if the local time cannot be read.
/// Unverified: written against the libc API, never run on Linux or macOS yet.
#[cfg(unix)]
pub fn local_minute_of_day() -> u16 {
    // SAFETY: `time` takes a null pointer, and `localtime_r` writes into `tm`, a valid zeroed struct.
    unsafe {
        let secs = libc::time(std::ptr::null_mut());
        let mut tm: libc::tm = std::mem::zeroed();
        if libc::localtime_r(&secs, &mut tm).is_null() {
            return 0;
        }
        ((tm.tm_hour * 60 + tm.tm_min) as u16).min(1439)
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_minute_of_the_day_is_within_a_day() {
        assert!(super::local_minute_of_day() < 1440);
    }
}
