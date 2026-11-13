// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Verify real show/hide acknowledgment ordering independently of OS scheduling.
use super::*;
use crate::{AccessibleControl, HitTarget, KeyAction, Pointer, PointerAction};
use std::{cell::RefCell, error::Error, num::NonZeroUsize, rc::Rc, sync::Arc, time::Duration};
use sugarsubstitute_contracts::{
    execution::{CompletionWake, Generation, SurfaceId},
    presentation::{FrameShape, PreparedFrame},
};

#[derive(Default)]
struct Content {
    visible: bool,
    calls: Rc<RefCell<Vec<&'static str>>>,
}
impl WindowContent for Content {
    fn start(&mut self, _: Arc<dyn CompletionWake>) -> Result<(), Box<dyn Error>> {
        Ok(())
    }
    fn request_frame(&mut self, _: FrameShape, _: Duration) -> Result<(), Box<dyn Error>> {
        Ok(())
    }
    fn poll_frame(&mut self, _: NonZeroUsize) -> Result<Option<PreparedFrame>, Box<dyn Error>> {
        Ok(None)
    }
    fn set_visible(&mut self, _: bool) -> Result<(), Box<dyn Error>> {
        Ok(())
    }
    fn request_shutdown(&mut self) -> Result<(), Box<dyn Error>> {
        Ok(())
    }
    fn poll_shutdown(&mut self) -> Result<bool, Box<dyn Error>> {
        Ok(true)
    }
    fn finish_shutdown(&mut self) -> Result<(), Box<dyn Error>> {
        Ok(())
    }
    fn hit_test(&self, _: Pointer) -> HitTarget {
        HitTarget::Client
    }
    fn pointer(&mut self, _: PointerAction, _: Pointer) -> bool {
        false
    }
    fn key(&mut self, _: KeyAction) -> bool {
        false
    }
    fn animating(&self) -> bool {
        false
    }
    fn accessible_controls(&self) -> &[AccessibleControl] {
        &[]
    }
    fn console(&self) -> Option<crate::ConsoleView<'_>> {
        None
    }
    fn activate(&mut self, _: u16) -> bool {
        false
    }
    fn frame_presented(&mut self, _: FrameIdentity) -> Result<(), Box<dyn Error>> {
        self.calls.borrow_mut().push("frame");
        Ok(())
    }
    fn presentation_visible(&self) -> bool {
        self.visible
    }
    fn presentation_hidden(&mut self) -> Result<(), Box<dyn Error>> {
        self.calls.borrow_mut().push("hidden");
        Ok(())
    }
}
fn identity() -> Result<FrameIdentity, Box<dyn Error>> {
    Ok(FrameIdentity {
        surface: SurfaceId::new(1)?,
        state_generation: Generation::new(1)?,
        sequence: Generation::new(1)?,
    })
}

#[test]
fn first_complete_frame_precedes_show_and_startup_notification() -> Result<(), Box<dyn Error>> {
    let mut visibility = Visibility::default();
    let mut content = Content { visible: true, ..Default::default() };
    let calls = content.calls.clone();
    visibility.reconcile(&mut content, |_| {
        calls.borrow_mut().push("early");
        Ok(())
    })?;
    assert!(calls.borrow().is_empty());
    assert!(visibility.preparing_first());
    visibility.submitted(&mut content, identity()?, |show| {
        assert!(show);
        calls.borrow_mut().push("show");
        Ok(())
    })?;
    assert_eq!(*calls.borrow(), ["show", "frame"]);
    assert!(visibility.shown());
    assert!(!visibility.preparing_first());
    Ok(())
}

#[test]
fn covered_child_submission_waits_for_authoritative_visibility_without_false_show()
-> Result<(), Box<dyn Error>> {
    let mut visibility = Visibility::default();
    let mut content = Content::default();
    let calls = content.calls.clone();
    visibility.submitted(&mut content, identity()?, |_| {
        calls.borrow_mut().push("early");
        Ok(())
    })?;
    assert_eq!(*calls.borrow(), ["frame"]);
    assert!(!visibility.shown());
    content.visible = true;
    visibility.reconcile(&mut content, |show| {
        assert!(show);
        calls.borrow_mut().push("show");
        Ok(())
    })?;
    assert_eq!(*calls.borrow(), ["frame", "show"]);
    assert!(visibility.shown());
    Ok(())
}

#[test]
fn hide_acknowledgment_follows_native_success_and_failed_hide_cannot_authorize_handoff()
-> Result<(), Box<dyn Error>> {
    let mut visibility = Visibility::default();
    let mut content = Content { visible: true, ..Default::default() };
    visibility.submitted(&mut content, identity()?, |_| Ok(()))?;
    content.calls.borrow_mut().clear();
    content.visible = false;
    assert!(
        visibility
            .reconcile(&mut content, |_| Err(DesktopError::message(
                "native_hide",
                "original native failure"
            )))
            .is_err()
    );
    assert!(visibility.shown());
    assert!(content.calls.borrow().is_empty());
    let calls = content.calls.clone();
    visibility.reconcile(&mut content, |show| {
        assert!(!show);
        calls.borrow_mut().push("hide");
        Ok(())
    })?;
    assert_eq!(*calls.borrow(), ["hide", "hidden"]);
    visibility.reconcile(&mut content, |_| {
        calls.borrow_mut().push("repeat");
        Ok(())
    })?;
    assert_eq!(*calls.borrow(), ["hide", "hidden"]);
    Ok(())
}

#[test]
fn failed_native_show_never_notifies_application_of_a_presented_frame() -> Result<(), Box<dyn Error>>
{
    let mut visibility = Visibility::default();
    let mut content = Content { visible: true, ..Default::default() };
    assert!(
        visibility
            .submitted(&mut content, identity()?, |_| Err(DesktopError::message(
                "native_show",
                "original show failure"
            )))
            .is_err()
    );
    assert!(!visibility.shown());
    assert!(!visibility.ever_shown());
    assert!(content.calls.borrow().is_empty());
    Ok(())
}
