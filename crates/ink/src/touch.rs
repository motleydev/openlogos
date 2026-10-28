//! Palm rejection and gesture arbitration as pure rules over a touch stream (B039).
//!
//! specs/13 "Ink interaction requirements": finger and stylus modes, palm-rejection testing,
//! "gestures separate drawing from scrolling; palm touches must not navigate another reading
//! pane". No stylus exists on the build machine, so the rules are stated here and tested over
//! synthetic streams; the Swift canvas and the web prototype mirror them (same case names) and
//! a device run later checks the mirrors against real touches. The arbiter never looks at
//! positions: a platform tells it which pane a touch began in and what kind of contact it is.

/// What produced a touch.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Contact {
    /// Apple Pencil, a Windows/Android pen, a `pointerType: "pen"` pointer.
    Stylus,
    /// A direct finger touch.
    Finger,
    /// A mouse or trackpad pointer.
    Pointer,
}

/// The reader's chosen input mode (a palette toggle, specs/13).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputMode {
    /// Only a stylus draws; fingers scroll and pan.
    Stylus,
    /// A finger draws in the ink pane; a stylus still draws.
    Finger,
}

/// The phase of one touch event.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    /// The contact began.
    Began,
    /// It moved.
    Moved,
    /// It lifted.
    Ended,
    /// The system cancelled it.
    Cancelled,
}

/// One touch event as the platform reports it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TouchEvent {
    /// Identity of the contact across its phases.
    pub id: u32,
    /// What the contact is.
    pub contact: Contact,
    /// The phase.
    pub phase: Phase,
    /// The pane the contact began in (`0` is the ink canvas; other panes are reading panes).
    pub pane: u32,
    /// The contact's major radius in points, when the platform reports one (`UITouch.majorRadius`,
    /// `PointerEvent.width / 2`); `None` for a stylus or a platform without it.
    pub radius: Option<f64>,
    /// Milliseconds on the platform's event clock.
    pub time_ms: f64,
}

/// What the canvas does with a touch event.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Decision {
    /// Feed the sample to the current stroke (or begin one).
    Draw,
    /// Scroll or pan the named pane.
    Scroll(u32),
    /// Do nothing with it, for this reason, for the rest of the touch's life.
    Ignore(IgnoreReason),
    /// The current stroke must be cancelled (its points discarded) before this event is handled
    /// as `Draw`: a stylus arrived while a finger was drawing.
    CancelStrokeThenDraw,
}

/// Why a touch is ignored.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IgnoreReason {
    /// A finger began while a stroke was in progress (the resting hand), in any pane.
    PalmDuringStroke,
    /// The contact is wider than a fingertip at the moment it began.
    LargeContact,
    /// A second finger during a finger-drawn stroke.
    SecondFinger,
    /// A finger in the ink pane in stylus mode: nothing to scroll there.
    FingerInStylusMode,
    /// A touch that was never seen beginning (missed `Began`).
    Unknown,
}

/// Contacts with a major radius at or above this (points) are palms at `Began`.
/// `UITouch.majorRadius` is about 5–12 pt for a fingertip on an iPad; a palm reports 20+.
pub const PALM_RADIUS_PT: f64 = 20.0;

/// The arbiter: tracks the active stroke and each live touch's decision.
#[derive(Debug, Clone, PartialEq)]
pub struct TouchArbiter {
    /// The chosen mode.
    pub mode: InputMode,
    /// The touch currently drawing, and whether it is a stylus.
    active_stroke: Option<(u32, Contact)>,
    /// Decisions by touch id, so later phases follow the first.
    live: Vec<(u32, Decision)>,
}

impl TouchArbiter {
    /// An arbiter in a mode with no touches down.
    #[must_use]
    pub const fn new(mode: InputMode) -> Self {
        Self {
            mode,
            active_stroke: None,
            live: Vec::new(),
        }
    }

    /// Whether a stroke is being drawn right now.
    #[must_use]
    pub fn stroke_in_progress(&self) -> bool {
        self.active_stroke.is_some()
    }

    fn decision_of(&self, id: u32) -> Option<Decision> {
        self.live.iter().find(|(i, _)| *i == id).map(|(_, d)| *d)
    }

    fn forget(&mut self, id: u32) {
        self.live.retain(|(i, _)| *i != id);
        if self.active_stroke.is_some_and(|(active, _)| active == id) {
            self.active_stroke = None;
        }
    }

    /// Decides one event. The decision for `Began` is remembered: `Moved`, `Ended` and
    /// `Cancelled` of the same touch get the same decision (a touch that began as a palm stays
    /// a palm even if it later moves like a finger), and an end releases the stroke.
    pub fn decide(&mut self, event: TouchEvent) -> Decision {
        match event.phase {
            Phase::Began => {
                let decision = self.decide_began(event);
                match decision {
                    Decision::Draw => self.active_stroke = Some((event.id, event.contact)),
                    Decision::CancelStrokeThenDraw => {
                        if let Some((previous, _)) = self.active_stroke.take() {
                            self.live.retain(|(i, _)| *i != previous);
                        }
                        self.active_stroke = Some((event.id, event.contact));
                    }
                    Decision::Scroll(_) | Decision::Ignore(_) => {}
                }
                let stored = if decision == Decision::CancelStrokeThenDraw {
                    Decision::Draw
                } else {
                    decision
                };
                self.live.push((event.id, stored));
                decision
            }
            Phase::Moved => self
                .decision_of(event.id)
                .unwrap_or(Decision::Ignore(IgnoreReason::Unknown)),
            Phase::Ended | Phase::Cancelled => {
                let decision = self
                    .decision_of(event.id)
                    .unwrap_or(Decision::Ignore(IgnoreReason::Unknown));
                self.forget(event.id);
                decision
            }
        }
    }

    fn decide_began(&self, event: TouchEvent) -> Decision {
        let in_canvas = event.pane == 0;
        match event.contact {
            // A stylus always draws in the canvas. It beats a finger that was drawing.
            Contact::Stylus => match self.active_stroke {
                Some((_, Contact::Finger)) if in_canvas => Decision::CancelStrokeThenDraw,
                Some(_) if in_canvas => Decision::Ignore(IgnoreReason::SecondFinger),
                None if in_canvas => Decision::Draw,
                _ => Decision::Scroll(event.pane),
            },
            // A pointer behaves like a stylus without pressure; there is no palm to reject.
            Contact::Pointer => {
                if in_canvas && self.active_stroke.is_none() {
                    Decision::Draw
                } else if in_canvas {
                    Decision::Ignore(IgnoreReason::SecondFinger)
                } else {
                    Decision::Scroll(event.pane)
                }
            }
            Contact::Finger => {
                if event.radius.is_some_and(|r| r >= PALM_RADIUS_PT) {
                    return Decision::Ignore(IgnoreReason::LargeContact);
                }
                // The rule the acceptance names: while a stroke is in progress, a finger
                // landing anywhere, another pane included, is the hand and does nothing.
                if self.active_stroke.is_some() {
                    return Decision::Ignore(if in_canvas && self.mode == InputMode::Finger {
                        IgnoreReason::SecondFinger
                    } else {
                        IgnoreReason::PalmDuringStroke
                    });
                }
                match (self.mode, in_canvas) {
                    (InputMode::Finger, true) => Decision::Draw,
                    (InputMode::Stylus, true) => Decision::Ignore(IgnoreReason::FingerInStylusMode),
                    (_, false) => Decision::Scroll(event.pane),
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ev(id: u32, contact: Contact, phase: Phase, pane: u32, t: f64) -> TouchEvent {
        TouchEvent {
            id,
            contact,
            phase,
            pane,
            radius: match contact {
                Contact::Finger => Some(7.0),
                _ => None,
            },
            time_ms: t,
        }
    }

    /// Runs a stream and returns the decisions in order.
    fn run(mode: InputMode, stream: &[TouchEvent]) -> Vec<Decision> {
        let mut arbiter = TouchArbiter::new(mode);
        stream.iter().map(|e| arbiter.decide(*e)).collect()
    }

    #[test]
    fn stylus_mode_finger_in_another_pane_during_a_stroke_does_not_scroll_it() {
        use Contact::{Finger, Stylus};
        use Phase::{Began, Ended, Moved};
        let stream = [
            ev(1, Stylus, Began, 0, 0.0),
            ev(1, Stylus, Moved, 0, 8.0),
            ev(2, Finger, Began, 2, 10.0), // the hand lands on reading pane 2
            ev(2, Finger, Moved, 2, 18.0),
            ev(1, Stylus, Moved, 0, 16.0),
            ev(2, Finger, Moved, 2, 26.0),
            ev(1, Stylus, Ended, 0, 24.0),
            ev(2, Finger, Moved, 2, 34.0), // still ignored after the pen lifted
            ev(2, Finger, Ended, 2, 40.0),
            ev(3, Finger, Began, 2, 50.0), // a fresh finger with no stroke scrolls
            ev(3, Finger, Moved, 2, 58.0),
        ];
        let d = run(InputMode::Stylus, &stream);
        assert_eq!(d[0], Decision::Draw);
        assert_eq!(d[2], Decision::Ignore(IgnoreReason::PalmDuringStroke));
        assert_eq!(d[3], Decision::Ignore(IgnoreReason::PalmDuringStroke));
        assert_eq!(d[5], Decision::Ignore(IgnoreReason::PalmDuringStroke));
        assert_eq!(
            d[7],
            Decision::Ignore(IgnoreReason::PalmDuringStroke),
            "a palm stays a palm"
        );
        assert_eq!(d[9], Decision::Scroll(2));
        assert_eq!(d[10], Decision::Scroll(2));
    }

    #[test]
    fn stylus_mode_finger_in_the_canvas_is_ignored_and_scrolls_elsewhere() {
        use Contact::Finger;
        use Phase::{Began, Ended};
        let d = run(
            InputMode::Stylus,
            &[
                ev(1, Finger, Began, 0, 0.0),
                ev(1, Finger, Ended, 0, 5.0),
                ev(2, Finger, Began, 1, 10.0),
            ],
        );
        assert_eq!(d[0], Decision::Ignore(IgnoreReason::FingerInStylusMode));
        assert_eq!(d[2], Decision::Scroll(1));
    }

    #[test]
    fn finger_mode_draws_with_one_finger_and_ignores_the_second() {
        use Contact::Finger;
        use Phase::{Began, Ended, Moved};
        let d = run(
            InputMode::Finger,
            &[
                ev(1, Finger, Began, 0, 0.0),
                ev(2, Finger, Began, 0, 4.0),
                ev(2, Finger, Moved, 0, 12.0),
                ev(1, Finger, Moved, 0, 8.0),
                ev(3, Finger, Began, 1, 9.0), // the other hand on a reading pane mid-stroke
                ev(1, Finger, Ended, 0, 16.0),
                ev(2, Finger, Ended, 0, 20.0),
                ev(4, Finger, Began, 1, 30.0),
            ],
        );
        assert_eq!(d[0], Decision::Draw);
        assert_eq!(d[1], Decision::Ignore(IgnoreReason::SecondFinger));
        assert_eq!(d[2], Decision::Ignore(IgnoreReason::SecondFinger));
        assert_eq!(d[3], Decision::Draw);
        assert_eq!(d[4], Decision::Ignore(IgnoreReason::PalmDuringStroke));
        assert_eq!(d[7], Decision::Scroll(1));
    }

    #[test]
    fn a_stylus_cancels_a_finger_stroke_and_takes_over() {
        use Contact::{Finger, Stylus};
        use Phase::{Began, Moved};
        let mut arbiter = TouchArbiter::new(InputMode::Finger);
        assert_eq!(arbiter.decide(ev(1, Finger, Began, 0, 0.0)), Decision::Draw);
        assert_eq!(
            arbiter.decide(ev(2, Stylus, Began, 0, 3.0)),
            Decision::CancelStrokeThenDraw
        );
        assert_eq!(
            arbiter.decide(ev(2, Stylus, Moved, 0, 11.0)),
            Decision::Draw
        );
        assert_eq!(
            arbiter.decide(ev(1, Finger, Moved, 0, 8.0)),
            Decision::Ignore(IgnoreReason::Unknown),
            "the cancelled finger is forgotten"
        );
        assert!(arbiter.stroke_in_progress());
    }

    #[test]
    fn a_wide_contact_is_a_palm_in_either_mode_even_with_no_stroke() {
        for mode in [InputMode::Stylus, InputMode::Finger] {
            let mut arbiter = TouchArbiter::new(mode);
            let palm = TouchEvent {
                radius: Some(PALM_RADIUS_PT),
                ..ev(9, Contact::Finger, Phase::Began, 0, 0.0)
            };
            assert_eq!(
                arbiter.decide(palm),
                Decision::Ignore(IgnoreReason::LargeContact)
            );
            let moved = TouchEvent {
                radius: Some(6.0), // narrows as the hand settles; still ignored
                ..ev(9, Contact::Finger, Phase::Moved, 0, 8.0)
            };
            assert_eq!(
                arbiter.decide(moved),
                Decision::Ignore(IgnoreReason::LargeContact)
            );
            assert!(!arbiter.stroke_in_progress());
        }
    }

    #[test]
    fn a_pointer_draws_in_the_canvas_and_scrolls_elsewhere() {
        use Contact::Pointer;
        use Phase::{Began, Ended};
        let d = run(
            InputMode::Stylus,
            &[
                ev(1, Pointer, Began, 0, 0.0),
                ev(1, Pointer, Ended, 0, 5.0),
                ev(2, Pointer, Began, 3, 10.0),
            ],
        );
        assert_eq!(d, [Decision::Draw, Decision::Draw, Decision::Scroll(3)]);
    }

    #[test]
    fn a_cancelled_stroke_releases_the_canvas() {
        use Contact::Stylus;
        use Phase::{Began, Cancelled};
        let mut arbiter = TouchArbiter::new(InputMode::Stylus);
        arbiter.decide(ev(1, Stylus, Began, 0, 0.0));
        assert!(arbiter.stroke_in_progress());
        arbiter.decide(ev(1, Stylus, Cancelled, 0, 4.0));
        assert!(!arbiter.stroke_in_progress());
        assert_eq!(arbiter.decide(ev(2, Stylus, Began, 0, 8.0)), Decision::Draw);
    }
}
