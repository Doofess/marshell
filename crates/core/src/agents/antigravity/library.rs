//! Where Antigravity CLI's rule files for library docs live and how they are written (slice 8a).

use crate::instructions::copy::{own_name_of, quote};
use crate::instructions::roots::Roots;
use std::path::{Path, PathBuf};

/// Antigravity CLI's cap on one rule file, in characters.
// Used by the library planner (slice 8a Task 5).
#[allow(dead_code)]
pub(crate) const RULE_CAP: usize = 24_000;

// Used by the library planner (slice 8a Task 5).
#[allow(dead_code)]
pub(crate) fn user_dir(roots: &Roots) -> PathBuf {
    roots.gemini.join("config").join("rules")
}

// Used by the library planner (slice 8a Task 5).
#[allow(dead_code)]
pub(crate) fn project_dir(project: &Path) -> PathBuf {
    project.join(".agents").join("rules")
}

// Used by the library planner (slice 8a Task 5).
#[allow(dead_code)]
pub(crate) fn file_name(name: &str) -> String {
    format!("marshell-{}.md", name.to_lowercase())
}

// Used by the library planner (slice 8a Task 5).
#[allow(dead_code)]
pub(crate) fn own_name(file_name: &str) -> Option<String> {
    own_name_of(file_name, ".md")
}

// Used by the library planner (slice 8a Task 5).
#[allow(dead_code)]
pub(crate) fn frontmatter(globs: &[String]) -> String {
    if globs.is_empty() {
        return "---\ntrigger: always_on\n---\n".to_string();
    }
    let joined: Vec<String> = globs
        .iter()
        .map(|g| {
            if g.starts_with("**/") {
                g.clone()
            } else {
                format!("**/{g}")
            }
        })
        .collect();
    format!("---\ntrigger: glob\nglobs: {}\n---\n", quote(&joined.join(", ")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn table_row() {
        let roots = Roots::under(Path::new("/h"));
        assert_eq!(
            user_dir(&roots),
            Path::new("/h").join(".gemini").join("config").join("rules")
        );
        assert_eq!(
            project_dir(Path::new("/p")),
            Path::new("/p").join(".agents").join("rules")
        );
        assert_eq!(file_name("Security"), "marshell-security.md");
        assert_eq!(own_name("marshell-a.md"), Some("a".to_string()));
        assert_eq!(own_name("style.md"), None);
        assert_eq!(frontmatter(&[]), "---\ntrigger: always_on\n---\n");
        let g = ["src/**".to_string(), "tests/**".to_string()];
        assert_eq!(
            frontmatter(&g),
            "---\ntrigger: glob\nglobs: '**/src/**, **/tests/**'\n---\n"
        );
        assert_eq!(
            frontmatter(&["**/a".to_string()]),
            "---\ntrigger: glob\nglobs: '**/a'\n---\n"
        );
        assert_eq!(RULE_CAP, 24_000);
    }
}
