//! GitHub, as the Changes panel's pull request bar knows it.

pub mod gh;
pub mod remote;

use crate::gitbox::refs;

/// Whether `b` is a branch name Marshell hands to git or GitHub: a valid ref under `refs/heads/` (which
/// `refs::valid_name` checks as a whole ref, never a bare branch), not starting with `-` (an option) or
/// `+` (a forced refspec), and holding nothing git reads as a revision (`~`, `^`, `@{`, a lone `@`), so
/// `refs/heads/<b>` on a push's left side can only ever name that branch.
pub fn valid_branch(b: &str) -> bool {
    !b.starts_with(['-', '+'])
        && !b.contains(['~', '^'])
        && !b.contains("@{")
        && b != "@"
        && !b.ends_with('.')
        && refs::valid_name(&format!("refs/heads/{b}"))
}

/// Whether a name may be written into a command a person or an agent may run:
/// `^[A-Za-z0-9._/][A-Za-z0-9._/-]*$`.
pub fn command_safe(name: &str) -> bool {
    let plain = |c: char| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '/');
    let mut chars = name.chars();
    chars.next().is_some_and(plain) && chars.all(|c| plain(c) || c == '-')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_branch_refuses_a_leading_dash_or_plus_and_bad_ref_names() {
        for bad in [
            "-x", "+main", "a..b", "a b", "", "a/", "/a", "a//b", ".a", "a.lock", "a:b",
        ] {
            assert!(!valid_branch(bad), "{bad:?}");
        }
        for good in ["feat/login", "v1.2", "main", "release/1.2-rc"] {
            assert!(valid_branch(good), "{good:?}");
        }
    }

    /// `refs/heads/a~1` on a push's left side would name the commit before `a`, not a branch.
    #[test]
    fn valid_branch_refuses_what_git_reads_as_a_revision() {
        for bad in ["a~1", "a^", "a^{commit}", "a@{1}", "@", "a@{upstream}"] {
            assert!(!valid_branch(bad), "{bad:?}");
        }
        assert!(valid_branch("user@feature"));
    }

    #[test]
    fn command_safe_refuses_shell_characters() {
        for bad in [
            "main;x", "x$(y)", "a|b", "a&b", "a'b", "a>b", "-x", "", "a b", "a\"b", "a`b", "a\nb",
        ] {
            assert!(!command_safe(bad), "{bad:?}");
        }
        for good in ["release/1.2", "main", "v1.2_x", ".hidden", "/abs", "a-b"] {
            assert!(command_safe(good), "{good:?}");
        }
    }
}
