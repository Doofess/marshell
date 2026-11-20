//! The window buttons' strip.
//!
//! While the window's right edge is dragged the page's Minimize, Maximize and Close trail the edge. On Windows a small
//! native window over them follows the edge instead, and draws the pictures the page sends (PLAN §9, Window-buttons strip
//! slice decisions).

// Used by the window in win.rs (Task 3).
#[allow(dead_code)]
pub mod pointer;
