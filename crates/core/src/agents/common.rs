//! Helpers the agents' conversation parsers share: truncation, paths, card wording and timestamps.

/// Cap on a tool card's detail, in characters.
pub const DETAIL_CAP: usize = 4_000;

/// "Created src/a.ts, 3 lines".
pub fn created_summary(path: &str, lines: usize) -> String {
    format!("Created {path}, {lines} {}", if lines == 1 { "line" } else { "lines" })
}

/// "Edited src/a.ts, 2 added, 1 removed".
pub fn edited_summary(path: &str, added: usize, removed: usize) -> String {
    format!("Edited {path}, {added} added, {removed} removed")
}

/// `s` cut to `max` characters, with `…` after the cut.
pub fn cap(s: &str, max: usize) -> String {
    match s.char_indices().nth(max) {
        Some((at, _)) => format!("{}…", &s[..at]),
        None => s.to_string(),
    }
}

/// `path` relative to `cwd` (with `/` separators) when it is inside it, else as given.
pub fn relative(path: &str, cwd: &str) -> String {
    let p = path.replace('\\', "/");
    let c = cwd.replace('\\', "/");
    let c = c.trim_end_matches('/');
    let inside = !c.is_empty()
        && p.len() > c.len()
        && p.is_char_boundary(c.len())
        && p[..c.len()].eq_ignore_ascii_case(c)
        && p.as_bytes()[c.len()] == b'/';
    if inside {
        p[c.len() + 1..].to_string()
    } else {
        path.to_string()
    }
}

/// An RFC 3339 timestamp (`2026-10-20T09:00:00.123Z`) as ms since the Unix epoch. `None` if it doesn't
/// read, or is before 1970.
pub fn rfc3339_ms(s: &str) -> Option<u64> {
    let b = s.as_bytes();
    if b.len() < 20 || b[4] != b'-' || b[7] != b'-' || !matches!(b[10], b'T' | b't') || b[13] != b':' || b[16] != b':' {
        return None;
    }
    let num = |from: usize, to: usize| -> Option<i64> {
        let digits = s.get(from..to)?;
        if digits.bytes().all(|c| c.is_ascii_digit()) {
            digits.parse().ok()
        } else {
            None
        }
    };
    let (year, month, day) = (num(0, 4)?, num(5, 7)?, num(8, 10)?);
    let (hour, minute, second) = (num(11, 13)?, num(14, 16)?, num(17, 19)?);
    let leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
    let month_days = match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if leap => 29,
        2 => 28,
        _ => return None,
    };
    if !(1..=month_days).contains(&day) || hour > 23 || minute > 59 || second > 60 {
        return None;
    }
    let mut rest = s.get(19..)?;
    let mut ms = 0;
    if let Some(frac) = rest.strip_prefix('.') {
        let digits = frac.bytes().take_while(u8::is_ascii_digit).count();
        if digits == 0 {
            return None;
        }
        let head = &frac[..digits.min(3)];
        ms = head.parse::<i64>().ok()? * 10_i64.pow(3 - head.len() as u32);
        rest = &frac[digits..];
    }
    let offset_minutes = match rest {
        "Z" | "z" => 0,
        _ => {
            let sign = match rest.as_bytes().first()? {
                b'+' => 1,
                b'-' => -1,
                _ => return None,
            };
            let (h, m) = (
                rest.get(1..3)?.parse::<i64>().ok()?,
                rest.get(4..6)?.parse::<i64>().ok()?,
            );
            if rest.len() != 6 || rest.as_bytes()[3] != b':' || h > 23 || m > 59 {
                return None;
            }
            sign * (h * 60 + m)
        }
    };
    // Days since 1970-01-01 from the civil date (Howard Hinnant's algorithm).
    let y = if month <= 2 { year - 1 } else { year };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let doy = (153 * (month + if month > 2 { -3 } else { 9 }) + 2) / 5 + day - 1;
    let days = era * 146_097 + yoe * 365 + yoe / 4 - yoe / 100 + doy - 719_468;
    let secs = days * 86_400 + hour * 3_600 + minute * 60 + second - offset_minutes * 60;
    u64::try_from(secs).ok().map(|s| s * 1_000 + ms as u64)
}
