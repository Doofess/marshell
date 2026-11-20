//! The window buttons' strip.
//!
//! While the window's right edge is dragged the page's Minimize, Maximize and Close trail the edge. On Windows a small
//! native window over them follows the edge instead, and draws the pictures the page sends (PLAN §9, Window-buttons strip
//! slice decisions).

// Only the window in win.rs uses the pointer's rules outside the tests.
#[cfg_attr(not(windows), allow(dead_code))]
pub mod pointer;
#[cfg(windows)]
mod win;

#[cfg(windows)]
pub use win::{install, on_resized};

use std::sync::Mutex;

/// The longest side, in device pixels, a picture may have. It keeps a bad payload from asking for a huge buffer.
pub const MAX_SIDE: u32 = 512;

/// One button's two pictures: at rest and under the pointer. RGBA bytes, rows top-down, `width * height * 4` each, where
/// the height is the set's.
#[derive(Debug, Clone, PartialEq, serde::Deserialize)]
pub struct ButtonPaint {
    pub width: u32,
    pub rest: Vec<u8>,
    pub hover: Vec<u8>,
}

/// Every picture the strip draws. Sizes are in device pixels.
#[derive(Debug, Clone, PartialEq, serde::Deserialize)]
pub struct Paint {
    pub height: u32,
    pub minimize: ButtonPaint,
    pub maximize: ButtonPaint,
    pub restore: ButtonPaint,
    pub close: ButtonPaint,
}

fn check_side(name: &str, what: &str, side: u32) -> Result<(), String> {
    if side == 0 || side > MAX_SIDE {
        return Err(format!("{name}'s {what} is {side}, not between 1 and {MAX_SIDE}"));
    }
    Ok(())
}

impl Paint {
    /// Refuses a set whose sizes don't agree, so the strip never reads past a buffer.
    pub fn check(&self) -> Result<(), String> {
        check_side("The strip", "height", self.height)?;
        for (name, button) in [
            ("Minimize", &self.minimize),
            ("Maximize", &self.maximize),
            ("Restore", &self.restore),
            ("Close", &self.close),
        ] {
            check_side(name, "width", button.width)?;
            let expected = button.width as usize * self.height as usize * 4;
            for (picture, bytes) in [("rest", &button.rest), ("hover", &button.hover)] {
                if bytes.len() != expected {
                    return Err(format!(
                        "{name}'s {picture} picture has {} bytes, expected {expected}",
                        bytes.len()
                    ));
                }
            }
        }
        if self.maximize.width != self.restore.width {
            return Err(format!(
                "Maximize is {} wide and Restore {}; they share one place",
                self.maximize.width, self.restore.width
            ));
        }
        Ok(())
    }

    /// The widths of Minimize, Maximize and Close.
    #[cfg_attr(not(windows), allow(dead_code))] // Only the Windows window reads the widths.
    pub fn widths(&self) -> [u32; 3] {
        [self.minimize.width, self.maximize.width, self.close.width]
    }
}

/// What the page last asked for.
#[derive(Default)]
pub struct Wanted {
    shown: bool,
    paint: Option<Paint>,
}

impl Wanted {
    /// Takes a request and returns the pictures to show, or `None` to hide the strip. A refused set is an error and
    /// leaves the last good one; a request to hide is kept even then. `paint: None` keeps the pictures it has.
    pub fn apply(&mut self, shown: bool, paint: Option<Paint>) -> Result<Option<&Paint>, String> {
        if let Some(paint) = paint {
            if let Err(refused) = paint.check() {
                // The page hides the strip when something covers it; that part of a request is never lost.
                if !shown {
                    self.shown = false;
                }
                return Err(refused);
            }
            self.paint = Some(paint);
        }
        self.shown = shown;
        Ok(self.showing())
    }

    /// The pictures the strip shows now, or `None` while it is hidden or has none.
    pub fn showing(&self) -> Option<&Paint> {
        if self.shown { self.paint.as_ref() } else { None }
    }
}

/// What the page last asked for, whatever window the request reached.
static WANTED: Mutex<Wanted> = Mutex::new(Wanted {
    shown: false,
    paint: None,
});

/// Gives the main window's strip its pictures and says whether it shows. `paint: None` keeps the pictures it has. On
/// Windows the strip's window then shows them or hides; elsewhere the request is only remembered.
#[tauri::command]
pub fn set_window_strip(window: tauri::Window, shown: bool, paint: Option<Paint>) -> Result<(), String> {
    if window.label() != "main" {
        return Err("only the main window has a strip".to_string());
    }
    let mut wanted = WANTED.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    // A refused set of pictures is reported after the strip is brought in line with what is still wanted: hidden, if the
    // request said so.
    let refused = wanted.apply(shown, paint).err();
    #[cfg(windows)]
    {
        if refused.is_none() || !shown {
            // `WANTED` is read only here, never by the window's procedure, so holding it across the call cannot deadlock.
            win::show(wanted.showing());
        }
    }
    refused.map_or(Ok(()), Err)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn button(width: u32, height: u32) -> ButtonPaint {
        let len = (width * height * 4) as usize;
        ButtonPaint {
            width,
            rest: vec![1; len],
            hover: vec![2; len],
        }
    }

    /// A valid set: `widths` are Minimize, Maximize, Close; Restore is as wide as Maximize.
    fn paint(height: u32, widths: [u32; 3]) -> Paint {
        Paint {
            height,
            minimize: button(widths[0], height),
            maximize: button(widths[1], height),
            restore: button(widths[1], height),
            close: button(widths[2], height),
        }
    }

    #[test]
    fn a_matching_set_passes() {
        assert_eq!(paint(40, [46, 46, 46]).check(), Ok(()));
    }

    #[test]
    fn a_picture_of_the_wrong_length_is_refused() {
        let mut p = paint(40, [46, 46, 46]);
        p.close.hover.pop();
        let err = p.check().unwrap_err();
        assert!(err.contains("Close"), "{err}");
        assert!(err.contains(&(46 * 40 * 4).to_string()), "{err}");
        assert!(err.contains(&(46 * 40 * 4 - 1).to_string()), "{err}");
    }

    #[test]
    fn a_side_of_zero_or_over_the_limit_is_refused() {
        let mut p = paint(40, [46, 46, 46]);
        p.height = 0;
        assert!(p.check().is_err());
        let mut p = paint(40, [46, 46, 46]);
        p.height = 513;
        assert!(p.check().is_err());
        let mut p = paint(40, [46, 46, 46]);
        p.minimize.width = 0;
        assert!(p.check().is_err());
        let mut p = paint(40, [46, 46, 46]);
        p.close.width = 513;
        assert!(p.check().is_err());
    }

    #[test]
    fn maximize_and_restore_must_be_as_wide() {
        let mut p = paint(40, [46, 46, 46]);
        p.restore = button(45, 40);
        assert!(p.check().is_err());
    }

    #[test]
    fn a_refused_set_leaves_the_last_good_one() {
        let good = paint(40, [46, 46, 46]);
        let mut bad = good.clone();
        bad.close.rest.pop();
        let mut wanted = Wanted::default();
        assert!(wanted.apply(true, Some(good.clone())).is_ok());
        assert!(wanted.apply(true, Some(bad)).is_err());
        assert_eq!(wanted.apply(true, None), Ok(Some(&good)));
    }

    #[test]
    fn a_hide_is_kept_when_the_pictures_sent_with_it_are_refused() {
        let good = paint(40, [46, 46, 46]);
        let mut bad = good.clone();
        bad.close.rest.pop();
        let mut wanted = Wanted::default();
        assert!(wanted.apply(true, Some(good.clone())).is_ok());
        // The page covers itself and sends new pictures in one request: the pictures are refused, the hide is not.
        assert!(wanted.apply(false, Some(bad.clone())).is_err());
        assert_eq!(wanted.showing(), None);
        // A refused set sent with "show" still changes nothing: a hidden strip stays hidden, a shown one stays shown.
        assert!(wanted.apply(true, Some(bad.clone())).is_err());
        assert_eq!(wanted.showing(), None);
        assert_eq!(wanted.apply(true, None), Ok(Some(&good)));
        assert!(wanted.apply(true, Some(bad)).is_err());
        assert_eq!(wanted.showing(), Some(&good));
    }

    #[test]
    fn nothing_shows_without_pictures_or_when_hidden() {
        let good = paint(40, [46, 46, 46]);
        let mut wanted = Wanted::default();
        assert_eq!(wanted.apply(true, None), Ok(None));
        assert_eq!(wanted.apply(false, Some(good)), Ok(None));
        assert!(matches!(wanted.apply(true, None), Ok(Some(_))));
    }
}
