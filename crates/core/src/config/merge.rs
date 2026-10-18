//! The 3-way merge for a file changed both by Marshell and outside it, through `diffy`.

/// Ok(merged) when the edits don't overlap; Err(text with <<<<<<< ======= >>>>>>> markers) when they do.
pub fn merge3(base: &str, ours: &str, theirs: &str) -> Result<String, String> {
    diffy::merge(base, ours, theirs)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn non_overlapping_edits_merge() {
        let merged = merge3("a\nb\nc\n", "A\nb\nc\n", "a\nb\nC\n");
        assert_eq!(merged, Ok("A\nb\nC\n".to_string()));
    }

    #[test]
    fn overlapping_edits_conflict_with_markers() {
        let merged = merge3("a\nb\nc\n", "a\nX\nc\n", "a\nY\nc\n");
        let text = merged.unwrap_err();
        assert!(text.contains("<<<<<<<"), "{text}");
        assert!(text.contains(">>>>>>>"), "{text}");
    }
}
