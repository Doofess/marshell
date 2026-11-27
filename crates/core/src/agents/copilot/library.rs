//! Where Copilot CLI's instruction files for library docs live and how they are written (slice 8a).

use crate::instructions::copy::{own_name_of, quote};
use crate::instructions::roots::Roots;
use std::path::{Path, PathBuf};

pub(crate) fn user_dir(roots: &Roots) -> PathBuf {
    roots.copilot.join("instructions")
}

pub(crate) fn project_dir(project: &Path) -> PathBuf {
    project.join(".github").join("instructions")
}

pub(crate) fn file_name(name: &str) -> String {
    format!("marshell-{}.instructions.md", name.to_lowercase())
}

pub(crate) fn own_name(file_name: &str) -> Option<String> {
    own_name_of(file_name, ".instructions.md")
}

pub(crate) fn frontmatter(globs: &[String]) -> String {
    if globs.is_empty() {
        return String::new();
    }
    format!("---\napplyTo: {}\n---\n", quote(&globs.join(", ")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn table_row() {
        let roots = Roots::under(Path::new("/h"));
        assert_eq!(user_dir(&roots), Path::new("/h").join(".copilot").join("instructions"));
        assert_eq!(
            project_dir(Path::new("/p")),
            Path::new("/p").join(".github").join("instructions")
        );
        assert_eq!(file_name("Security"), "marshell-security.instructions.md");
        assert_eq!(own_name("marshell-a.md"), None);
        assert_eq!(own_name("marshell-a.instructions.md"), Some("a".to_string()));
        assert_eq!(own_name("style.md"), None);
        assert_eq!(frontmatter(&[]), "");
        let g = ["src/**".to_string(), "tests/**".to_string()];
        assert_eq!(frontmatter(&g), "---\napplyTo: 'src/**, tests/**'\n---\n");
    }
}
