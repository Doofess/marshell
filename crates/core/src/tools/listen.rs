//! The TCP listeners of this machine, and which of them are a tab's own dev servers.
//!
//! Only loopback or unspecified local addresses count: a server bound to a LAN address alone doesn't answer on
//! `localhost`, so a chip for it would open nothing. Nothing here logs a port or a URL.

/// The ports in `listeners` (pid, local port) owned by a process of the tab's `tree` (pid, name) that is not in
/// `agent_pids`, ascending and without repeats. Empty when `quiet`: the command's own port lines are transient (a login
/// callback), so its listener makes no chip either.
pub fn chips_for(listeners: &[(u32, u16)], tree: &[(u32, String)], agent_pids: &[u32], quiet: bool) -> Vec<u16> {
    if quiet {
        return Vec::new();
    }
    let mut ports: Vec<u16> = listeners
        .iter()
        .filter(|(pid, _)| tree.iter().any(|(p, _)| p == pid) && !agent_pids.contains(pid))
        .map(|&(_, port)| port)
        .collect();
    ports.sort_unstable();
    ports.dedup();
    ports
}

/// The pid and local port of every TCP listener on a loopback or unspecified address (`127.0.0.0/8`, `0.0.0.0`, `::1`,
/// `::`), IPv4 and IPv6. `None` when the table cannot be read or the platform has none. Blocking.
pub fn listeners() -> Option<Vec<(u32, u16)>> {
    #[cfg(windows)]
    {
        windows_listeners()
    }
    #[cfg(target_os = "linux")]
    {
        linux_listeners()
    }
    #[cfg(not(any(windows, target_os = "linux")))]
    {
        None
    }
}

/// An IPv4 address (octets in memory order) a local server answers `localhost` on.
#[cfg(any(windows, test))]
fn v4_local(octets: &[u8]) -> bool {
    octets.first() == Some(&127) || octets.iter().all(|&b| b == 0)
}

/// An IPv6 address (16 bytes in memory order) that is `::1` or `::`.
#[cfg(any(windows, test))]
fn v6_local(octets: &[u8]) -> bool {
    octets.len() == 16 && octets[..15].iter().all(|&b| b == 0) && octets[15] <= 1
}

/// Size of a `MIB_TCPROW_OWNER_PID` and the offsets of the fields read from it.
#[cfg(any(windows, test))]
mod v4 {
    pub const ROW: usize = 24;
    pub const ADDR: usize = 4;
    pub const PORT: usize = 8;
    pub const PID: usize = 20;
}

/// The same for a `MIB_TCP6ROW_OWNER_PID`.
#[cfg(any(windows, test))]
mod v6 {
    pub const ROW: usize = 56;
    pub const ADDR: usize = 0;
    pub const PORT: usize = 20;
    pub const PID: usize = 52;
}

/// The listeners in a `GetExtendedTcpTable` buffer: a `u32` row count, then the rows. `None` when the count does not
/// fit the buffer. The local port is network byte order in the low 16 bits of its `u32`, which is the first two bytes
/// in memory; an address is its bytes in memory order.
#[cfg(any(windows, test))]
fn parse_table(bytes: &[u8], v6_rows: bool) -> Option<Vec<(u32, u16)>> {
    let (row, addr, port, pid) = if v6_rows {
        (v6::ROW, v6::ADDR, v6::PORT, v6::PID)
    } else {
        (v4::ROW, v4::ADDR, v4::PORT, v4::PID)
    };
    let count = u32::from_ne_bytes(bytes.get(..4)?.try_into().ok()?) as usize;
    let end = count.checked_mul(row)?.checked_add(4)?;
    let rows = bytes.get(4..end)?;
    let mut out = Vec::new();
    for r in rows.chunks_exact(row) {
        let local = if v6_rows {
            v6_local(&r[addr..addr + 16])
        } else {
            v4_local(&r[addr..addr + 4])
        };
        if local {
            let p = u16::from_be_bytes([r[port], r[port + 1]]);
            let owner = u32::from_ne_bytes(r[pid..pid + 4].try_into().ok()?);
            out.push((owner, p));
        }
    }
    Some(out)
}

#[cfg(windows)]
fn windows_listeners() -> Option<Vec<(u32, u16)>> {
    use windows_sys::Win32::Foundation::{ERROR_INSUFFICIENT_BUFFER, NO_ERROR};
    use windows_sys::Win32::NetworkManagement::IpHelper::{GetExtendedTcpTable, TCP_TABLE_OWNER_PID_LISTENER};
    use windows_sys::Win32::Networking::WinSock::{AF_INET, AF_INET6};

    let mut all = Vec::new();
    for (family, v6_rows) in [(AF_INET, false), (AF_INET6, true)] {
        let mut size: u32 = 0;
        // Words, so the buffer is aligned for the table the API writes into it.
        let mut buffer: Vec<u64> = Vec::new();
        let mut done = false;
        // The table can grow between the call that sizes it and the call that reads it.
        for _ in 0..4 {
            let ptr = if buffer.is_empty() {
                std::ptr::null_mut()
            } else {
                buffer.as_mut_ptr().cast()
            };
            // SAFETY: `ptr` is null with `size` 0, or points to `size` writable bytes (the buffer is at least that
            // long, see below); `size` is a valid u32.
            let status =
                unsafe { GetExtendedTcpTable(ptr, &mut size, 0, u32::from(family), TCP_TABLE_OWNER_PID_LISTENER, 0) };
            if status == NO_ERROR {
                done = true;
                break;
            }
            if status != ERROR_INSUFFICIENT_BUFFER {
                return None;
            }
            // A little room for listeners that open meanwhile.
            let bytes = (size as usize).checked_add(512)?;
            if bytes > 64 << 20 {
                return None;
            }
            buffer = vec![0u64; bytes.div_ceil(8)];
            size = u32::try_from(buffer.len() * 8).ok()?;
        }
        if !done {
            return None;
        }
        let capacity = buffer.len() * 8;
        let used = (size as usize).min(capacity);
        // SAFETY: the buffer holds `capacity` initialized bytes (zeroed words), and `used` is within it.
        let bytes = unsafe { std::slice::from_raw_parts(buffer.as_ptr().cast::<u8>(), used) };
        all.extend(parse_table(bytes, v6_rows)?);
    }
    Some(all)
}

/// The listeners in the text of `/proc/net/tcp` (`v6` false) or `/proc/net/tcp6` as `(local port, socket inode)`,
/// for state `0A` (LISTEN) on a loopback or unspecified address. Addresses are hex words in native (little endian) order.
#[cfg(any(target_os = "linux", test))]
fn parse_proc_net(text: &str, v6: bool) -> Vec<(u16, u64)> {
    let mut out = Vec::new();
    for line in text.lines().skip(1) {
        let fields: Vec<&str> = line.split_whitespace().collect();
        if fields.len() < 10 || fields[3] != "0A" {
            continue;
        }
        let Some((addr, port)) = fields[1].split_once(':') else {
            continue;
        };
        let Ok(port) = u16::from_str_radix(port, 16) else {
            continue;
        };
        let Some(octets) = hex_words(addr) else {
            continue;
        };
        let local = if v6 { v6_local(&octets) } else { v4_local(&octets) };
        let Ok(inode) = fields[9].parse::<u64>() else {
            continue;
        };
        if local && inode != 0 {
            out.push((port, inode));
        }
    }
    out
}

/// The bytes of an address written as 8-digit hex words, each a little-endian `u32`.
#[cfg(any(target_os = "linux", test))]
fn hex_words(text: &str) -> Option<Vec<u8>> {
    if !text.len().is_multiple_of(8) || !text.is_ascii() {
        return None;
    }
    let mut out = Vec::new();
    for i in (0..text.len()).step_by(8) {
        out.extend(u32::from_str_radix(&text[i..i + 8], 16).ok()?.to_le_bytes());
    }
    Some(out)
}

#[cfg(target_os = "linux")]
fn linux_listeners() -> Option<Vec<(u32, u16)>> {
    let mut sockets: Vec<(u16, u64)> = Vec::new();
    for (path, v6) in [("/proc/net/tcp", false), ("/proc/net/tcp6", true)] {
        sockets.extend(parse_proc_net(&std::fs::read_to_string(path).ok()?, v6));
    }
    let mut out = Vec::new();
    // A process that exits or belongs to another user during the scan is skipped.
    for entry in std::fs::read_dir("/proc").ok()?.flatten() {
        let Some(pid) = entry.file_name().to_str().and_then(|n| n.parse::<u32>().ok()) else {
            continue;
        };
        let Ok(fds) = std::fs::read_dir(format!("/proc/{pid}/fd")) else {
            continue;
        };
        for fd in fds.flatten() {
            let Ok(target) = std::fs::read_link(fd.path()) else {
                continue;
            };
            let target = target.to_string_lossy();
            let Some(inode) = target
                .strip_prefix("socket:[")
                .and_then(|t| t.strip_suffix(']'))
                .and_then(|t| t.parse::<u64>().ok())
            else {
                continue;
            };
            out.extend(
                sockets
                    .iter()
                    .filter(|(_, i)| *i == inode)
                    .map(|&(port, _)| (pid, port)),
            );
        }
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(any(windows, target_os = "linux"))]
    #[test]
    fn listeners_finds_a_socket_this_test_opened() {
        let socket = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = socket.local_addr().unwrap().port();
        let found = listeners().expect("the listener table");
        assert!(found.contains(&(std::process::id(), port)), "{port} not in {found:?}");
        // And an IPv6 loopback one, when the machine has IPv6.
        if let Ok(socket6) = std::net::TcpListener::bind("[::1]:0") {
            let port6 = socket6.local_addr().unwrap().port();
            let found = listeners().expect("the listener table");
            assert!(found.contains(&(std::process::id(), port6)), "{port6} not in {found:?}");
        }
    }

    #[cfg(windows)]
    #[test]
    fn the_row_layouts_match_the_windows_structs() {
        use std::mem::{offset_of, size_of};
        use windows_sys::Win32::NetworkManagement::IpHelper::{MIB_TCP6ROW_OWNER_PID, MIB_TCPROW_OWNER_PID};
        assert_eq!(size_of::<MIB_TCPROW_OWNER_PID>(), v4::ROW);
        assert_eq!(offset_of!(MIB_TCPROW_OWNER_PID, dwLocalAddr), v4::ADDR);
        assert_eq!(offset_of!(MIB_TCPROW_OWNER_PID, dwLocalPort), v4::PORT);
        assert_eq!(offset_of!(MIB_TCPROW_OWNER_PID, dwOwningPid), v4::PID);
        assert_eq!(size_of::<MIB_TCP6ROW_OWNER_PID>(), v6::ROW);
        assert_eq!(offset_of!(MIB_TCP6ROW_OWNER_PID, ucLocalAddr), v6::ADDR);
        assert_eq!(offset_of!(MIB_TCP6ROW_OWNER_PID, dwLocalPort), v6::PORT);
        assert_eq!(offset_of!(MIB_TCP6ROW_OWNER_PID, dwOwningPid), v6::PID);
    }

    fn row4(addr: [u8; 4], port: u16, pid: u32) -> Vec<u8> {
        let mut r = vec![0u8; v4::ROW];
        r[v4::ADDR..v4::ADDR + 4].copy_from_slice(&addr);
        r[v4::PORT..v4::PORT + 2].copy_from_slice(&port.to_be_bytes());
        r[v4::PID..v4::PID + 4].copy_from_slice(&pid.to_ne_bytes());
        r
    }

    fn table(count: u32, rows: &[Vec<u8>]) -> Vec<u8> {
        let mut t = count.to_ne_bytes().to_vec();
        t.extend(rows.iter().flatten());
        t
    }

    #[test]
    fn a_table_keeps_loopback_and_unspecified_listeners_only() {
        let rows = [
            row4([127, 0, 0, 1], 3000, 10),
            row4([0, 0, 0, 0], 8080, 11),
            row4([127, 1, 2, 3], 9000, 12),
            row4([192, 168, 1, 5], 8787, 13),
        ];
        assert_eq!(
            parse_table(&table(4, &rows), false),
            Some(vec![(10, 3000), (11, 8080), (12, 9000)])
        );
        // A count larger than the buffer is refused, not indexed.
        assert_eq!(parse_table(&table(5, &rows), false), None);
        assert_eq!(parse_table(&table(u32::MAX, &rows), false), None);
        assert_eq!(parse_table(&[1, 0], false), None);
        assert_eq!(parse_table(&table(0, &[]), true), Some(Vec::new()));
        let mut loopback = vec![0u8; v6::ROW];
        loopback[v6::ADDR + 15] = 1;
        loopback[v6::PORT..v6::PORT + 2].copy_from_slice(&5173u16.to_be_bytes());
        loopback[v6::PID..v6::PID + 4].copy_from_slice(&7u32.to_ne_bytes());
        let mut lan = loopback.clone();
        lan[v6::ADDR] = 0xfe;
        lan[v6::ADDR + 1] = 0x80;
        assert_eq!(parse_table(&table(2, &[loopback, lan]), true), Some(vec![(7, 5173)]));
    }

    #[test]
    fn proc_net_text_gives_listening_sockets_on_local_addresses() {
        let v4 = "  sl  local_address rem_address   st tx_queue rx_queue tr tm->when retrnsmt   uid  timeout inode\n\
   0: 0100007F:0BB8 00000000:0000 0A 00000000:00000000 00:00000000 00000000  1000        0 111 1 0 100 0 0 10 0\n\
   1: 0100007F:0BB9 0100007F:C000 01 00000000:00000000 00:00000000 00000000  1000        0 222 1 0 100 0 0 10 0\n\
   2: 0500A8C0:1F40 00000000:0000 0A 00000000:00000000 00:00000000 00000000  1000        0 333 1 0 100 0 0 10 0\n\
   3: 00000000:1F90 00000000:0000 0A 00000000:00000000 00:00000000 00000000  1000        0 444 1 0 100 0 0 10 0\n";
        assert_eq!(parse_proc_net(v4, false), [(3000, 111), (8080, 444)]);
        let v6 = "  sl  local_address                         remote_address                        st tx_queue rx_queue tr tm->when retrnsmt   uid  timeout inode\n\
   0: 00000000000000000000000001000000:1435 00000000000000000000000000000000:0000 0A 00000000:00000000 00:00000000 00000000  1000        0 555 1 0 100 0 0 10 0\n\
   1: 000080FE000000000000000001000000:1436 00000000000000000000000000000000:0000 0A 00000000:00000000 00:00000000 00000000  1000        0 666 1 0 100 0 0 10 0\n";
        assert_eq!(parse_proc_net(v6, true), [(5173, 555)]);
    }
}
