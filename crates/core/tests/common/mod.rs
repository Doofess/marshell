use std::sync::Once;

/// Keep test children in a kill-on-close job, including when a test panics.
/// On other platforms the app's job helper does nothing.
pub fn tie_children_to_test() {
    static INIT: Once = Once::new();
    INIT.call_once(|| marshell_core::job::tie_children_to_app().expect("could not tie children to the test process"));
}
