//! Where Claude Code's rule files for library docs live and how they are written (slice 8a).

use crate::instructions::copy::{own_name_of, quote};
use crate::instructions::roots::Roots;
use std::path::{Path, PathBuf};

// Used by the library planner (slice 8a Task 5).
#[allow(dead_code)]
pub(crate) fn user_dir(roots: &Roots) -> PathBuf {
    roots.claude.join("rules")
}

// Used by the library planner (slice 8a Task 5).
#[allow(dead_code)]
pub(crate) fn project_dir(project: &Path) -> PathBuf {
    project.join(".claude").join("rules")
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
        return String::new();
    }
    let items: String = globs.iter().map(|g| format!("  - {}\n", quote(g))).collect();
    format!("---\npaths:\n{items}---\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn table_row() {
        let roots = Roots::under(Path::new("/h"));
        assert_eq!(user_dir(&roots), Path::new("/h").join(".claude").join("rules"));
        assert_eq!(
            project_dir(Path::new("/p")),
            Path::new("/p").join(".claude").join("rules")
        );
        assert_eq!(file_name("Security"), "marshell-security.md");
        assert_eq!(own_name("marshell-a.md"), Some("a".to_string()));
        assert_eq!(own_name("style.md"), None);
        assert_eq!(frontmatter(&[]), "");
        let g = ["src/**".to_string(), "tests/**".to_string()];
        assert_eq!(frontmatter(&g), "---\npaths:\n  - 'src/**'\n  - 'tests/**'\n---\n");
    }
}
