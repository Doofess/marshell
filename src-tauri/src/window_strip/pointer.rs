//! The strip's pointer rules, apart from any window.
//!
//! The strip's window turns mouse messages into `Event`s; `Pointer` says which button is drawn lit and when a press and
//! release count as a click.

/// One of the three buttons the strip holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Button {
    Minimize,
    Maximize,
    Close,
}

/// The button at `x` from the strip's left edge; `widths` are Minimize, Maximize, Close in device pixels.
pub fn hit(x: i32, widths: [u32; 3]) -> Option<Button> {
    if x < 0 {
        return None;
    }
    let x = x as i64;
    let mut edge = 0i64;
    for (width, button) in widths.iter().zip([Button::Minimize, Button::Maximize, Button::Close]) {
        edge += i64::from(*width);
        if x < edge {
            return Some(button);
        }
    }
    None
}

/// What the strip's window saw the pointer do. `Move`, `Down` and `Up` carry the button under the pointer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Event {
    Move(Option<Button>),
    Leave,
    Down(Button),
    Up(Option<Button>),
}

/// Which button the pointer is over and which one it went down on.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Pointer {
    hover: Option<Button>,
    pressed: Option<Button>,
}

/// What an event asks of the window: paint again, and act for a button.
#[derive(Debug, PartialEq, Eq)]
pub struct Outcome {
    pub redraw: bool,
    pub act: Option<Button>,
}

impl Pointer {
    /// A pointer over nothing. `const`, so the window's state can sit in a `static`.
    pub const fn new() -> Self {
        Self {
            hover: None,
            pressed: None,
        }
    }

    /// Takes one event. `redraw` is true exactly when `lit()` changed; `act` names the button a press and release
    /// on the same button completes.
    pub fn on(&mut self, event: Event) -> Outcome {
        let before = self.lit();
        let mut act = None;
        match event {
            Event::Move(over) => self.hover = over,
            Event::Leave => {
                self.hover = None;
                self.pressed = None;
            }
            Event::Down(button) => {
                self.hover = Some(button);
                self.pressed = Some(button);
            }
            Event::Up(over) => {
                if over.is_some() && over == self.pressed {
                    act = over;
                }
                self.hover = over;
                self.pressed = None;
            }
        }
        Outcome {
            redraw: self.lit() != before,
            act,
        }
    }

    /// The one button drawn with its hover picture, if any. A pressed button is lit only while the pointer is on it.
    pub fn lit(&self) -> Option<Button> {
        match self.pressed {
            Some(_) if self.hover != self.pressed => None,
            _ => self.hover,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const W: [u32; 3] = [58, 57, 58];

    #[test]
    fn hit_finds_the_button_by_its_column() {
        assert_eq!(hit(0, W), Some(Button::Minimize));
        assert_eq!(hit(57, W), Some(Button::Minimize));
        assert_eq!(hit(58, W), Some(Button::Maximize));
        assert_eq!(hit(114, W), Some(Button::Maximize));
        assert_eq!(hit(115, W), Some(Button::Close));
        assert_eq!(hit(172, W), Some(Button::Close));
        assert_eq!(hit(-1, W), None);
        assert_eq!(hit(173, W), None);
    }

    #[test]
    fn a_press_and_release_on_one_button_acts_once() {
        let mut p = Pointer::default();
        p.on(Event::Down(Button::Close));
        assert_eq!(p.on(Event::Up(Some(Button::Close))).act, Some(Button::Close));
        assert_eq!(p.on(Event::Up(Some(Button::Close))).act, None);
    }

    #[test]
    fn a_release_on_another_button_or_outside_does_nothing() {
        let mut p = Pointer::default();
        p.on(Event::Down(Button::Minimize));
        assert_eq!(p.on(Event::Up(Some(Button::Close))).act, None);
        p.on(Event::Down(Button::Minimize));
        assert_eq!(p.on(Event::Up(None)).act, None);
    }

    #[test]
    fn leaving_forgets_the_press() {
        let mut p = Pointer::default();
        p.on(Event::Down(Button::Maximize));
        p.on(Event::Leave);
        assert_eq!(p.on(Event::Up(Some(Button::Maximize))).act, None);
    }

    #[test]
    fn only_a_change_asks_for_a_redraw() {
        let mut p = Pointer::default();
        assert!(p.on(Event::Move(Some(Button::Minimize))).redraw);
        assert!(!p.on(Event::Move(Some(Button::Minimize))).redraw);
        let mut q = Pointer::default();
        assert!(!q.on(Event::Leave).redraw);
    }

    #[test]
    fn a_pressed_button_is_lit_only_under_the_pointer() {
        let mut p = Pointer::default();
        p.on(Event::Down(Button::Close));
        assert_eq!(p.lit(), Some(Button::Close));
        p.on(Event::Move(Some(Button::Minimize)));
        assert_eq!(p.lit(), None);
        p.on(Event::Move(Some(Button::Close)));
        assert_eq!(p.lit(), Some(Button::Close));
    }
}
