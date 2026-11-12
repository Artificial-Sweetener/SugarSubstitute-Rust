// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Portable feedback ordering remains deterministic without a native window or rendering.
use super::*;
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

struct Owner {
    feedback: Feedback,
    events: Rc<RefCell<Vec<&'static str>>>,
    render_blocked: bool,
}

impl FeedbackOwner for Owner {
    fn apply_input(&mut self, input: Input) -> Result<(), Box<dyn Error>> {
        self.events.borrow_mut().push(match input {
            Input::Unfocused => "unfocused",
            _ => "input",
        });
        self.feedback = match input {
            Input::Unfocused => Feedback { cursor: CursorIntent::Arrow, capture: false },
            _ => Feedback { cursor: CursorIntent::ClosedHand, capture: true },
        };
        Ok(())
    }

    fn current_feedback(&self) -> Feedback {
        assert!(self.render_blocked);
        self.events.borrow_mut().push("feedback");
        self.feedback
    }
}

struct Target {
    events: Rc<RefCell<Vec<&'static str>>>,
    cursor: Cell<Option<CursorIntent>>,
    capture: Cell<bool>,
    reject_once: Cell<bool>,
}

impl FeedbackTarget for Target {
    fn publish_cursor(&self, cursor: CursorIntent) -> Result<(), Box<dyn Error>> {
        self.events.borrow_mut().push("cursor");
        self.cursor.set(Some(cursor));
        Ok(())
    }

    fn reconcile_capture(&self, capture: bool) -> Result<bool, Box<dyn Error>> {
        self.events.borrow_mut().push("capture");
        if capture && self.reject_once.replace(false) {
            return Ok(false);
        }
        self.capture.set(capture);
        Ok(true)
    }
}

fn boundary(
    feedback: Feedback,
    reject_once: bool,
) -> (Owner, Target, Rc<RefCell<Vec<&'static str>>>) {
    let events = Rc::new(RefCell::new(Vec::new()));
    (
        Owner { feedback, events: events.clone(), render_blocked: true },
        Target {
            events: events.clone(),
            cursor: Cell::new(None),
            capture: Cell::new(false),
            reject_once: Cell::new(reject_once),
        },
        events,
    )
}

#[test]
fn input_publishes_cursor_and_capture_without_polling_or_requesting_a_frame()
-> Result<(), Box<dyn Error>> {
    let (mut owner, target, events) =
        boundary(Feedback { cursor: CursorIntent::Arrow, capture: false }, false);
    dispatch_input(&mut owner, Some(&target), Input::TemporaryNavigation(true))?;
    assert_eq!(&*events.borrow(), &["input", "feedback", "cursor", "capture"]);
    assert_eq!(target.cursor.get(), Some(CursorIntent::ClosedHand));
    assert!(target.capture.get());
    Ok(())
}

#[test]
fn rejected_capture_cancels_once_and_republishes_released_feedback() -> Result<(), Box<dyn Error>> {
    let (mut owner, target, events) =
        boundary(Feedback { cursor: CursorIntent::OpenHand, capture: false }, true);
    dispatch_input(&mut owner, Some(&target), Input::TemporaryNavigation(true))?;
    assert_eq!(
        &*events.borrow(),
        &["input", "feedback", "cursor", "capture", "unfocused", "feedback", "cursor", "capture"]
    );
    assert_eq!(target.cursor.get(), Some(CursorIntent::Arrow));
    assert!(!target.capture.get());
    Ok(())
}

#[test]
fn initial_empty_feedback_is_cached_and_an_unavailable_native_target_is_inert()
-> Result<(), Box<dyn Error>> {
    let empty = Feedback { cursor: CursorIntent::Arrow, capture: false };
    let (mut owner, target, events) = boundary(empty, false);
    synchronize(&mut owner, Some(&target))?;
    assert_eq!(&*events.borrow(), &["feedback", "cursor", "capture"]);
    assert_eq!(target.cursor.get(), Some(CursorIntent::Arrow));
    assert!(!target.capture.get());

    events.borrow_mut().clear();
    synchronize(&mut owner, None::<&Target>)?;
    assert!(events.borrow().is_empty());
    Ok(())
}
