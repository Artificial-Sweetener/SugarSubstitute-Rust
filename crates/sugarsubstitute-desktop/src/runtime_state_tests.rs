// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise native preparation/closing policy with deterministic owned-content boundaries.

use super::*;
use crate::{AccessibleControl, HitTarget, KeyAction, Pointer, PointerAction};
use std::{
    collections::VecDeque,
    error::Error,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};
use sugarsubstitute_contracts::{
    execution::{CompletionWake, Generation, MemoryLease, MemoryRelease, SurfaceId},
    presentation::{FrameIdentity, FrameValidator},
};

#[derive(Default)]
struct Content {
    calls: Vec<&'static str>,
    frames: VecDeque<PreparedFrame>,
    shutdown: VecDeque<Result<bool, Box<dyn Error>>>,
    request_error: bool,
    shutdown_error: bool,
    last_budget: usize,
}

impl WindowContent for Content {
    fn start(&mut self, _: Arc<dyn CompletionWake>) -> Result<(), Box<dyn Error>> {
        Ok(())
    }
    fn request_frame(&mut self, _: FrameShape, _: Duration) -> Result<(), Box<dyn Error>> {
        self.calls.push("prepare");
        if self.request_error {
            return Err(std::io::Error::other("original admission cause").into());
        }
        Ok(())
    }
    fn poll_frame(&mut self, max: NonZeroUsize) -> Result<Option<PreparedFrame>, Box<dyn Error>> {
        self.calls.push("ack");
        self.last_budget = max.get();
        Ok(self.frames.pop_front())
    }
    fn set_visible(&mut self, visible: bool) -> Result<(), Box<dyn Error>> {
        self.calls.push(if visible { "show" } else { "hide" });
        Ok(())
    }
    fn request_shutdown(&mut self) -> Result<(), Box<dyn Error>> {
        self.calls.push("cancel");
        if self.shutdown_error {
            return Err(std::io::Error::other("shutdown request cause").into());
        }
        Ok(())
    }
    fn poll_shutdown(&mut self) -> Result<bool, Box<dyn Error>> {
        self.calls.push("settle");
        self.shutdown.pop_front().unwrap_or(Ok(false))
    }
    fn finish_shutdown(&mut self) -> Result<(), Box<dyn Error>> {
        self.calls.push("finish");
        Ok(())
    }
    fn hit_test(&self, _: Pointer) -> HitTarget {
        HitTarget::Client
    }
    fn pointer(&mut self, _: PointerAction, _: Pointer) -> bool {
        true
    }
    fn key(&mut self, _: KeyAction) -> bool {
        true
    }
    fn animating(&self) -> bool {
        true
    }
    fn accessible_controls(&self) -> &[AccessibleControl] {
        &[]
    }
    fn activate(&mut self, _: u16) -> bool {
        true
    }
}

struct Release(Arc<AtomicUsize>);
impl MemoryRelease for Release {
    fn release(&self, _: u64) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }
}

fn shape() -> Result<FrameShape, Box<dyn Error>> {
    Ok(FrameShape::new(2, 1, 1.0)?)
}
fn budget() -> Result<NonZeroUsize, Box<dyn Error>> {
    Ok(NonZeroUsize::new(32).ok_or("Nonzero budget")?)
}
fn frame(shape: FrameShape, released: &Arc<AtomicUsize>) -> Result<PreparedFrame, Box<dyn Error>> {
    let pixels = vec![0; shape.byte_len()];
    let lease = MemoryLease::new(pixels.capacity() as u64, Box::new(Release(released.clone())));
    let identity = FrameIdentity {
        surface: SurfaceId::new(1)?,
        state_generation: Generation::new(1)?,
        sequence: Generation::new(1)?,
    };
    let mut validator = FrameValidator::new(shape, identity, pixels, lease)?;
    if !validator.validate_next(budget()?)? {
        return Err("Fixture validation incomplete".into());
    }
    Ok(validator.finish()?)
}

#[test]
fn input_and_animation_admit_preparation_without_replacing_or_exposing_retained_storage()
-> Result<(), Box<dyn Error>> {
    let shape = shape()?;
    let released = Arc::new(AtomicUsize::new(0));
    let mut content = Content::default();
    let mut surface = SurfaceState::new();
    surface.visibility(&mut content, true);
    content.frames.push_back(frame(shape, &released)?);
    assert!(surface.poll(&mut content, Some(shape), budget()?, Duration::ZERO));
    let pointer = surface.exposable(Some(shape)).ok_or("No retained frame")?.pixels().as_ptr();
    surface.prepare(&mut content, Some(shape), Duration::ZERO, false);
    surface.prepare(&mut content, Some(shape), Duration::ZERO, false);
    surface.prepare(&mut content, Some(shape), Duration::ZERO, true);
    surface.dirty();
    surface.prepare(&mut content, Some(shape), Duration::ZERO, false);
    assert_eq!(content.calls.iter().filter(|call| **call == "prepare").count(), 3);
    assert_eq!(
        surface.exposable(Some(shape)).ok_or("Retained frame lost")?.pixels().as_ptr(),
        pointer
    );
    assert_eq!(released.load(Ordering::SeqCst), 0);
    assert_eq!(content.last_budget, 32);
    Ok(())
}

#[test]
fn hidden_surface_invalidates_before_new_work_and_still_disposes_terminal_frames()
-> Result<(), Box<dyn Error>> {
    let shape = shape()?;
    let released = Arc::new(AtomicUsize::new(0));
    let mut content = Content::default();
    let mut surface = SurfaceState::new();
    surface.visibility(&mut content, true);
    surface.visibility(&mut content, false);
    surface.dirty();
    surface.prepare(&mut content, Some(shape), Duration::ZERO, true);
    content.frames.push_back(frame(shape, &released)?);
    assert!(!surface.poll(&mut content, Some(shape), budget()?, Duration::ZERO));
    assert_eq!(released.load(Ordering::SeqCst), 1);
    assert!(surface.exposable(Some(shape)).is_none());
    surface.visibility(&mut content, true);
    surface.prepare(&mut content, Some(shape), Duration::ZERO, false);
    assert_eq!(content.calls, ["show", "hide", "ack", "show", "prepare"]);
    Ok(())
}

#[test]
fn mismatched_completions_are_acknowledged_and_native_expose_requires_actual_shape()
-> Result<(), Box<dyn Error>> {
    let shape = shape()?;
    let different = FrameShape::new(1, 2, 1.0)?;
    let released = Arc::new(AtomicUsize::new(0));
    let mut content = Content::default();
    let mut surface = SurfaceState::new();
    surface.visibility(&mut content, true);
    content.frames.push_back(frame(shape, &released)?);
    assert!(surface.poll(&mut content, Some(shape), budget()?, Duration::ZERO));
    content.frames.push_back(frame(different, &released)?);
    assert!(!surface.poll(&mut content, Some(shape), budget()?, Duration::ZERO));
    assert_eq!(released.load(Ordering::SeqCst), 1);
    assert!(surface.exposable(Some(different)).is_none());
    assert!(surface.exposable(None).is_none());
    assert!(surface.exposable(Some(shape)).is_some());
    surface.close(&mut content);
    assert!(surface.exposable(Some(shape)).is_none());
    assert_eq!(released.load(Ordering::SeqCst), 1);
    drop(surface);
    assert_eq!(released.load(Ordering::SeqCst), 2);
    Ok(())
}

#[test]
fn changed_visible_geometry_is_requested_before_completion_publication()
-> Result<(), Box<dyn Error>> {
    let initial = shape()?;
    let resized = FrameShape::new(3, 2, 1.5)?;
    let released = Arc::new(AtomicUsize::new(0));
    let mut content = Content::default();
    let mut surface = SurfaceState::new();
    surface.visibility(&mut content, true);
    content.frames.push_back(frame(initial, &released)?);
    assert!(surface.poll(&mut content, Some(initial), budget()?, Duration::ZERO));
    content.frames.push_back(frame(initial, &released)?);
    assert!(!surface.poll(&mut content, Some(resized), budget()?, Duration::ZERO));
    assert_eq!(content.calls, ["show", "prepare", "ack", "prepare", "ack"]);
    assert_eq!(surface.requested_shape, Some(resized));
    assert!(surface.exposable(Some(resized)).is_none());
    assert_eq!(released.load(Ordering::SeqCst), 1);
    content.frames.push_back(frame(resized, &released)?);
    assert!(surface.poll(&mut content, Some(resized), budget()?, Duration::ZERO));
    assert_eq!(content.calls.last(), Some(&"ack"));
    assert_eq!(content.calls.iter().filter(|call| **call == "prepare").count(), 2);
    assert_eq!(released.load(Ordering::SeqCst), 2);
    Ok(())
}

#[test]
fn closing_is_idempotent_and_waits_for_actual_settlement_after_errors() -> Result<(), Box<dyn Error>>
{
    let shape = shape()?;
    let released = Arc::new(AtomicUsize::new(0));
    let mut content = Content { shutdown_error: true, ..Content::default() };
    content.shutdown.extend([
        Ok(false),
        Err(std::io::Error::other("later poll cause").into()),
        Ok(true),
    ]);
    let mut surface = SurfaceState::new();
    surface.visibility(&mut content, true);
    surface.close(&mut content);
    surface.close(&mut content);
    surface.dirty();
    surface.visibility(&mut content, true);
    surface.prepare(&mut content, Some(shape), Duration::ZERO, true);
    content.frames.push_back(frame(shape, &released)?);
    assert!(!surface.poll(&mut content, Some(shape), budget()?, Duration::ZERO));
    assert_eq!(released.load(Ordering::SeqCst), 1);
    surface.poll_shutdown(&mut content);
    assert!(!surface.settled);
    surface.poll_shutdown(&mut content);
    assert!(!surface.settled);
    surface.poll_shutdown(&mut content);
    assert!(surface.settled);
    surface.poll_shutdown(&mut content);
    assert_eq!(content.calls, ["show", "hide", "cancel", "ack", "settle", "settle", "settle"]);
    assert_eq!(
        surface.failure.as_ref().ok_or("Missing error")?.to_string(),
        "request_shutdown: shutdown request cause"
    );
    Ok(())
}

#[test]
fn request_error_retains_its_original_cause_while_shutdown_failures_do_not_replace_it()
-> Result<(), Box<dyn Error>> {
    let mut content = Content { request_error: true, shutdown_error: true, ..Content::default() };
    let mut surface = SurfaceState::new();
    surface.visibility(&mut content, true);
    surface.prepare(&mut content, Some(shape()?), Duration::ZERO, false);
    assert!(surface.closing);
    assert!(!surface.settled);
    let error = surface.failure.as_ref().ok_or("Original cause lost")?;
    assert_eq!(error.to_string(), "request_frame: original admission cause");
    assert_eq!(
        error
            .source()
            .and_then(|cause| cause.downcast_ref::<std::io::Error>())
            .ok_or("Cause type lost")?
            .to_string(),
        "original admission cause"
    );
    assert_eq!(content.calls, ["show", "prepare", "hide", "cancel"]);
    Ok(())
}
