//! The Marshell home is made owner-only on every start: a protected DACL on Windows, modes 0700/0600 on Unix.

use marshell_core::private::{HomeAcl, secure_home};

#[cfg(windows)]
use marshell_core::private::{current_user_sid, dacl_of};

#[cfg(windows)]
fn expected_sids() -> Vec<String> {
    let mut want = vec![current_user_sid().unwrap(), "S-1-5-18".to_string()];
    want.sort();
    want
}

#[cfg(windows)]
#[test]
fn home_gets_a_protected_dacl_for_the_user_and_system() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path().join("home");
    std::fs::create_dir_all(home.join("sub")).unwrap();
    std::fs::write(home.join("sub").join("f.txt"), "x").unwrap();

    assert_eq!(secure_home(&home).unwrap(), HomeAcl::Fixed);

    let acl = dacl_of(&home).unwrap();
    assert!(acl.protected);
    let mut sids = acl.allow_sids.clone();
    sids.sort();
    assert_eq!(sids, expected_sids());

    let child = dacl_of(&home.join("sub").join("f.txt")).unwrap();
    let mut sids = child.allow_sids.clone();
    sids.sort();
    assert_eq!(sids, expected_sids());
    assert!(
        child.all_inherited,
        "an existing child goes back to inheriting the home's ACL"
    );
}

#[cfg(windows)]
#[test]
fn a_file_created_later_inherits_it() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path().join("home");
    secure_home(&home).unwrap();

    let file = home.join("later.json");
    std::fs::write(&file, "{}").unwrap();
    let sub = home.join("later-dir");
    std::fs::create_dir(&sub).unwrap();

    for path in [&file, &sub] {
        let acl = dacl_of(path).unwrap();
        let mut sids = acl.allow_sids.clone();
        sids.sort();
        assert_eq!(sids, expected_sids(), "{}", path.display());
        assert!(acl.all_inherited, "{}", path.display());
    }
}

#[test]
fn second_call_is_already_private() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path().join("home");
    assert_eq!(secure_home(&home).unwrap(), HomeAcl::Fixed);
    assert!(home.is_dir(), "the home is created when missing");
    assert_eq!(secure_home(&home).unwrap(), HomeAcl::AlreadyPrivate);
}

#[cfg(unix)]
#[test]
fn modes_are_0700_and_0600_and_executables_keep_x() {
    use std::os::unix::fs::{PermissionsExt, symlink};

    let mode = |p: &std::path::Path| std::fs::symlink_metadata(p).unwrap().permissions().mode() & 0o777;
    let set = |p: &std::path::Path, m: u32| std::fs::set_permissions(p, std::fs::Permissions::from_mode(m)).unwrap();

    let dir = tempfile::tempdir().unwrap();
    let home = dir.path().join("home");
    let sub = home.join("sub");
    std::fs::create_dir_all(&sub).unwrap();
    let plain = sub.join("plain.txt");
    let script = sub.join("run.sh");
    std::fs::write(&plain, "x").unwrap();
    std::fs::write(&script, "x").unwrap();
    set(&plain, 0o644);
    set(&script, 0o755);
    set(&sub, 0o755);
    set(&home, 0o755);

    // A link to something outside the home is never followed.
    let outside = dir.path().join("outside.txt");
    std::fs::write(&outside, "x").unwrap();
    set(&outside, 0o644);
    symlink(&outside, sub.join("link")).unwrap();

    assert_eq!(secure_home(&home).unwrap(), HomeAcl::Fixed);
    assert_eq!(mode(&home), 0o700);
    assert_eq!(mode(&sub), 0o700);
    assert_eq!(mode(&plain), 0o600);
    assert_eq!(mode(&script), 0o700);
    assert_eq!(mode(&outside), 0o644, "the link target is left alone");
}
