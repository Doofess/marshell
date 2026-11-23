//! Which GitHub repository a checkout belongs to, and where `git push origin` would really go.
//!
//! The address comes from the config the sandbox read (`gitbox::Origin`) and is parsed strictly: only the
//! forms below name a repository, and anything else (a local path, `file://`, a UNC path, `ext::`) names none.

use crate::gitbox::Origin;

/// A repository on a host. It holds no user, password or port: they are dropped from the address.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteRepo {
    pub host: String,
    pub owner: String,
    pub name: String,
}

/// The schemes whose address is `scheme://[user[:password]@]host[:port]/owner/name`.
const SCHEMES: &[&str] = &["https", "http", "ssh", "git+ssh", "ssh+git", "git"];

/// The hosts that are `github.com`.
const GITHUB_COM: &[&str] = &["github.com", "ssh.github.com", "www.github.com"];

/// Whether `host` is a name made of dot-separated labels of letters, digits and hyphens.
fn valid_host(host: &str) -> bool {
    host.len() <= 253
        && host.split('.').all(|label| {
            !label.is_empty()
                && !label.starts_with('-')
                && !label.ends_with('-')
                && label.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
        })
}

/// `owner/name`, with one optional trailing `/` and one optional `.git`, and nothing else.
fn parse_path(path: &str) -> Option<(String, String)> {
    let path = path.strip_suffix('/').unwrap_or(path);
    let path = path.strip_suffix(".git").unwrap_or(path);
    let (owner, name) = path.split_once('/')?;
    let owner_ok = (1..=39).contains(&owner.len()) && owner.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-');
    let name_ok = (1..=100).contains(&name.len())
        && name != "."
        && name != ".."
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-'));
    (owner_ok && name_ok).then(|| (owner.to_string(), name.to_string()))
}

/// `owner/name` as GitHub's `nameWithOwner` writes it, and nothing else: no `.git`, no trailing `/`, no third part.
/// The owner and the name follow the rules of an address's path. GitHub's text is only display text until it passes.
pub(crate) fn slug(text: &str) -> Option<(String, String)> {
    let (owner, name) = parse_path(text)?;
    (format!("{owner}/{name}") == text).then_some((owner, name))
}

/// The host of `authority` (`[user[:password]@]host[:port]`), lowercased, with the user and port dropped.
/// An authority with a backslash, `#`, `?`, a space or a control character is refused: tools disagree on
/// where its host ends.
fn parse_authority(authority: &str) -> Option<String> {
    if authority
        .chars()
        .any(|c| c.is_control() || c.is_whitespace() || matches!(c, '\\' | '#' | '?'))
    {
        return None;
    }
    let host_port = authority.rsplit_once('@').map_or(authority, |(_, rest)| rest);
    let host = match host_port.rsplit_once(':') {
        Some((host, port)) => {
            if port.is_empty() || port.len() > 5 || !port.bytes().all(|b| b.is_ascii_digit()) {
                return None;
            }
            host
        }
        None => host_port,
    };
    let host = host.to_ascii_lowercase();
    valid_host(&host).then_some(host)
}

/// The repository an address names, or None when it isn't one of the accepted forms.
///
/// Accepted: `https://` and `http://` (a `user[:password]@` part and a port are dropped), `ssh://`,
/// `git+ssh://`, `ssh+git://` and `git://` (a user and a port are dropped), and the scp form
/// `[user@]host:owner/name` whose host is longer than one character (so a drive letter is never a host).
/// The path is exactly `owner/name`, with one optional `.git` and one optional trailing `/`.
/// `ssh.github.com` and `www.github.com` are `github.com`.
pub fn parse_remote(url: &str) -> Option<RemoteRepo> {
    let colon = url.find(':')?;
    let (host, path) = if url[colon..].starts_with("://") {
        if !SCHEMES.contains(&&url[..colon]) {
            return None;
        }
        let (authority, path) = url[colon + 3..].split_once('/')?;
        (parse_authority(authority)?, path)
    } else {
        let (target, path) = (&url[..colon], &url[colon + 1..]);
        let host = match target.rsplit_once('@') {
            Some((user, host)) => {
                let user_ok = !user.is_empty()
                    && user
                        .bytes()
                        .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-'));
                user_ok.then_some(host)?
            }
            None => target,
        };
        if host.len() < 2 {
            return None;
        }
        let host = host.to_ascii_lowercase();
        (valid_host(&host).then_some(host)?, path)
    };
    let (owner, name) = parse_path(path)?;
    let host = if GITHUB_COM.contains(&host.as_str()) {
        "github.com".to_string()
    } else {
        host
    };
    Some(RemoteRepo { host, owner, name })
}

/// `url` with the longest matching `insteadOf` prefix replaced by its base, as git does it (the first of
/// equal lengths wins). `rules` are `(base, insteadOf)`. None when no rule matches.
pub fn apply_instead_of(url: &str, rules: &[(String, String)]) -> Option<String> {
    let mut best: Option<&(String, String)> = None;
    for rule in rules {
        if url.starts_with(rule.1.as_str()) && best.is_none_or(|b| b.1.len() < rule.1.len()) {
            best = Some(rule);
        }
    }
    best.map(|(base, instead)| format!("{base}{}", &url[instead.len()..]))
}

/// The repository `origin` fetches from: its first url, with `insteadOf` applied.
pub fn fetch_repo(origin: &Origin) -> Option<RemoteRepo> {
    let url = origin.urls.first()?;
    let url = apply_instead_of(url, &origin.instead_of).unwrap_or_else(|| url.clone());
    parse_remote(&url)
}

/// The repository `git push origin` goes to, when that is one that can be named. Git's own rule
/// (`alias_all_urls` in `remote.c`): several urls or pushurls push to several places, which is refused; one
/// pushurl is used with `insteadOf` applied (`pushInsteadOf` doesn't touch it); with none, the url is
/// rewritten by a matching `pushInsteadOf`, else by `insteadOf`. An address that names no repository is refused.
#[allow(clippy::result_unit_err)]
pub fn push_repo(origin: &Origin) -> Result<RemoteRepo, ()> {
    if origin.urls.len() > 1 || origin.push_urls.len() > 1 {
        return Err(());
    }
    let address = match (origin.push_urls.first(), origin.urls.first()) {
        (Some(push), _) => apply_instead_of(push, &origin.instead_of).unwrap_or_else(|| push.clone()),
        (None, Some(url)) => apply_instead_of(url, &origin.push_instead_of)
            .or_else(|| apply_instead_of(url, &origin.instead_of))
            .unwrap_or_else(|| url.clone()),
        (None, None) => return Err(()),
    };
    parse_remote(&address).ok_or(())
}

/// The GitHub host `host` is: `github.com` for it and its two aliases, another host only when the person is
/// signed in to it (`signed_in`, from `gh`). The answer is lowercase.
pub fn github_host(host: &str, signed_in: &[String]) -> Option<String> {
    let host = host.to_ascii_lowercase();
    if GITHUB_COM.contains(&host.as_str()) {
        return Some("github.com".to_string());
    }
    signed_in.iter().any(|h| h.eq_ignore_ascii_case(&host)).then_some(host)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn repo(host: &str, owner: &str, name: &str) -> RemoteRepo {
        RemoteRepo {
            host: host.into(),
            owner: owner.into(),
            name: name.into(),
        }
    }

    fn widgets() -> RemoteRepo {
        repo("github.com", "octo-org", "widgets")
    }

    fn origin(urls: &[&str], push_urls: &[&str]) -> Origin {
        Origin {
            urls: urls.iter().map(|s| s.to_string()).collect(),
            push_urls: push_urls.iter().map(|s| s.to_string()).collect(),
            ..Origin::default()
        }
    }

    fn rules(pairs: &[(&str, &str)]) -> Vec<(String, String)> {
        pairs.iter().map(|(a, b)| (a.to_string(), b.to_string())).collect()
    }

    #[test]
    fn parses_https_with_and_without_dot_git() {
        assert_eq!(parse_remote("https://github.com/octo-org/widgets.git"), Some(widgets()));
        assert_eq!(parse_remote("https://github.com/octo-org/widgets"), Some(widgets()));
        assert_eq!(parse_remote("http://github.com/octo-org/widgets"), Some(widgets()));
        assert_eq!(
            parse_remote("https://github.com/octo-org/widgets.git/"),
            Some(widgets())
        );
        assert_eq!(
            parse_remote("https://github.com/octo-org/my.repo_1.git"),
            Some(repo("github.com", "octo-org", "my.repo_1"))
        );
        assert_eq!(
            parse_remote("https://ghe.example.com:8443/octo-org/widgets"),
            Some(repo("ghe.example.com", "octo-org", "widgets"))
        );
    }

    #[test]
    fn parses_scp_like_ssh_and_skips_a_drive_letter() {
        assert_eq!(parse_remote("git@github.com:octo-org/widgets.git"), Some(widgets()));
        assert_eq!(parse_remote("github.com:octo-org/widgets"), Some(widgets()));
        assert_eq!(parse_remote("C:/work/widgets"), None);
        assert_eq!(parse_remote("c:widgets/x"), None);
        assert_eq!(parse_remote(r"C:\repo"), None);
        assert_eq!(parse_remote("git@github.com:/octo-org/widgets"), None);
    }

    #[test]
    fn parses_ssh_url_with_port_and_ssh_github_com() {
        let expected = Some(widgets());
        assert_eq!(parse_remote("ssh://git@ssh.github.com:443/octo-org/widgets"), expected);
        assert_eq!(parse_remote("ssh://git@github.com/octo-org/widgets.git"), expected);
        assert_eq!(parse_remote("git+ssh://git@github.com/octo-org/widgets"), expected);
        assert_eq!(parse_remote("ssh+git://git@github.com/octo-org/widgets"), expected);
        assert_eq!(parse_remote("git://github.com:9418/octo-org/widgets"), expected);
        assert_eq!(parse_remote("https://www.github.com/octo-org/widgets"), expected);
        // Not a port, so not this form.
        assert_eq!(parse_remote("ssh://git@github.com:octo-org/widgets"), None);
        assert_eq!(parse_remote("ssh://git@github.com:/octo-org/widgets"), None);
    }

    #[test]
    fn drops_user_and_password_from_https() {
        let got = parse_remote("https://u:secret@GitHub.com/octo-org/widgets/").unwrap();
        assert_eq!(got, widgets());
        assert!(!format!("{got:?}").contains("secret"));
        assert!(!format!("{got:?}").contains("u:"));
    }

    #[test]
    fn refuses_local_paths_and_file_urls() {
        for url in [
            "../widgets",
            "/srv/git/widgets.git",
            "file:///c:/x/y",
            "//server/share/o/n",
            r"\\server\share\o\n",
            "",
            "widgets",
            "ext::sh -c x",
            "ext::sh -c x/y",
            "ftp://github.com/octo-org/widgets",
            "HTTPS://github.com/octo-org/widgets",
            "fd::3/x",
        ] {
            assert_eq!(parse_remote(url), None, "{url:?}");
        }
    }

    #[test]
    fn refuses_extra_segments_and_bad_names() {
        for url in [
            "https://github.com/a/b/c",
            "https://github.com/octo-org",
            "https://github.com/octo-org/",
            "https://github.com//widgets",
            "https://github.com/octo-org/..",
            "https://github.com/octo-org/.",
            "https://github.com/octo-org/.git",
            "https://github.com/oct_o/widgets",
            "https://github.com/octo-org/wid gets",
            "https://github.com/octo-org/wid%20gets",
            "https://github.com/octo-org/widgets?x=1",
            "https://github.com/octo-org/widgets#x",
            "https://github.com:/octo-org/widgets",
            "https://github.com:abc/octo-org/widgets",
            "https://[::1]/octo-org/widgets",
            "https:///octo-org/widgets",
            "git@github.com:a/b/c",
            "git@github.com:octo-org",
        ] {
            assert_eq!(parse_remote(url), None, "{url:?}");
        }
        let long_owner = format!("https://github.com/{}/widgets", "a".repeat(40));
        assert_eq!(parse_remote(&long_owner), None);
        let long_name = format!("https://github.com/octo-org/{}", "a".repeat(101));
        assert_eq!(parse_remote(&long_name), None);
        let ok_name = format!("https://github.com/octo-org/{}", "a".repeat(100));
        assert!(parse_remote(&ok_name).is_some());
    }

    #[test]
    fn instead_of_takes_the_longest_prefix() {
        let both = rules(&[
            ("https://github.com/", "gh:"),
            ("https://github.com/octo-org/", "gh:oo/"),
        ]);
        assert_eq!(
            apply_instead_of("gh:oo/widgets", &both).as_deref(),
            Some("https://github.com/octo-org/widgets")
        );
        assert_eq!(
            apply_instead_of("gh:other/x", &both).as_deref(),
            Some("https://github.com/other/x")
        );
        assert_eq!(apply_instead_of("https://example.com/x", &both), None);
        assert_eq!(apply_instead_of("x", &[]), None);
    }

    #[test]
    fn github_host_matches_the_github_com_family_and_signed_in_hosts() {
        assert_eq!(github_host("github.com", &[]).as_deref(), Some("github.com"));
        assert_eq!(github_host("ssh.github.com", &[]).as_deref(), Some("github.com"));
        assert_eq!(github_host("ghe.example.com", &[]), None);
        let signed_in = vec!["ghe.example.com".to_string()];
        assert_eq!(
            github_host("ghe.example.com", &signed_in).as_deref(),
            Some("ghe.example.com")
        );
        assert_eq!(github_host("gitlab.com", &signed_in), None);
        assert_eq!(github_host("gitlab.com", &[]), None);
    }

    #[test]
    fn push_goes_where_fetch_does_with_one_url() {
        let o = origin(&["https://github.com/octo-org/widgets.git"], &[]);
        assert_eq!(fetch_repo(&o), Some(widgets()));
        assert_eq!(push_repo(&o), Ok(widgets()));
        assert_eq!(push_repo(&o), Ok(fetch_repo(&o).unwrap()));
        assert_eq!(fetch_repo(&Origin::default()), None);
        assert_eq!(push_repo(&Origin::default()), Err(()));
    }

    #[test]
    fn fetch_applies_instead_of_to_the_first_url() {
        let o = Origin {
            urls: vec!["gh:octo-org/widgets".into()],
            instead_of: rules(&[("https://github.com/", "gh:")]),
            ..Origin::default()
        };
        assert_eq!(fetch_repo(&o), Some(widgets()));
        assert_eq!(push_repo(&o), Ok(widgets()));
    }

    #[test]
    fn a_pushurl_to_another_repository_does_not_match() {
        let o = origin(
            &["https://github.com/octo-org/widgets.git"],
            &["https://github.com/octo-org/other.git"],
        );
        let pushed = push_repo(&o).unwrap();
        assert_eq!(pushed, repo("github.com", "octo-org", "other"));
        assert_ne!(Some(pushed), fetch_repo(&o));
    }

    #[test]
    fn a_pushurl_to_a_local_path_is_refused() {
        for push in ["C:/bare.git", "../bare", "ext::sh -c x", "/srv/bare.git"] {
            let o = origin(&["https://github.com/octo-org/widgets"], &[push]);
            assert_eq!(push_repo(&o), Err(()), "{push:?}");
        }
    }

    #[test]
    fn two_urls_or_two_pushurls_are_refused() {
        let two_urls = origin(
            &[
                "https://github.com/octo-org/widgets",
                "https://github.com/octo-org/widgets",
            ],
            &[],
        );
        assert_eq!(push_repo(&two_urls), Err(()));
        let two_push = origin(
            &["https://github.com/octo-org/widgets"],
            &[
                "https://github.com/octo-org/widgets",
                "https://github.com/octo-org/widgets",
            ],
        );
        assert_eq!(push_repo(&two_push), Err(()));
    }

    #[test]
    fn push_instead_of_applies_only_without_a_pushurl() {
        let mut o = Origin {
            urls: vec!["https://github.com/octo-org/widgets".into()],
            push_instead_of: rules(&[("git@github.com:", "https://github.com/")]),
            ..Origin::default()
        };
        // The rule rewrites the url for push: still the same repository, so it matches.
        assert_eq!(push_repo(&o), Ok(widgets()));
        // A rule whose result is another repository shows that it is the one used.
        o.push_instead_of = rules(&[("https://github.com/other-org/", "https://github.com/octo-org/")]);
        assert_eq!(push_repo(&o), Ok(repo("github.com", "other-org", "widgets")));
        // With a pushurl the rule is ignored.
        o.push_urls = vec!["https://github.com/octo-org/widgets".into()];
        assert_eq!(push_repo(&o), Ok(widgets()));
        // pushInsteadOf beats insteadOf when it matches, and insteadOf is the fallback.
        let o = Origin {
            urls: vec!["https://github.com/octo-org/widgets".into()],
            instead_of: rules(&[("https://github.com/fetch-org/", "https://github.com/octo-org/")]),
            push_instead_of: rules(&[("https://github.com/push-org/", "https://github.com/octo-org/")]),
            ..Origin::default()
        };
        assert_eq!(fetch_repo(&o), Some(repo("github.com", "fetch-org", "widgets")));
        assert_eq!(push_repo(&o), Ok(repo("github.com", "push-org", "widgets")));
        let o = Origin {
            push_instead_of: rules(&[("https://nowhere/", "x")]),
            ..o
        };
        assert_eq!(push_repo(&o), Ok(repo("github.com", "fetch-org", "widgets")));
    }

    #[test]
    fn instead_of_applies_to_a_pushurl_but_push_instead_of_does_not() {
        let o = Origin {
            urls: vec!["https://github.com/octo-org/widgets".into()],
            push_urls: vec!["gh:octo-org/widgets".into()],
            instead_of: rules(&[("https://github.com/", "gh:")]),
            push_instead_of: rules(&[("https://example.com/", "gh:")]),
        };
        assert_eq!(push_repo(&o), Ok(widgets()));
    }

    #[test]
    fn an_ssh_pushurl_of_the_same_repository_matches() {
        let o = origin(
            &["https://github.com/octo-org/widgets"],
            &["git@github.com:octo-org/widgets.git"],
        );
        assert_eq!(push_repo(&o), Ok(widgets()));
        assert_eq!(push_repo(&o).ok(), fetch_repo(&o));
    }
}
