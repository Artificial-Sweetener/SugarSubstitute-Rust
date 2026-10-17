// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Fault native effects while exercising the actual retained document publication owner.
use super::*;
use crate::console::index::tests::{TestResult, index, source};
use std::{error::Error, sync::atomic::Ordering};
struct Port {
    observation: Observation,
    fail: Option<&'static str>,
    calls: Vec<&'static str>,
    restored: Option<(i32, i32, i32, bool)>,
}
impl Port {
    fn new(observation: Observation) -> Self {
        Self { observation, fail: None, calls: vec![], restored: None }
    }
    fn effect(&mut self, step: &'static str) -> Result<(), DesktopError> {
        self.calls.push(step);
        if self.fail == Some(step) {
            return Err(DesktopError::new("original_console_effect", std::io::Error::other(step)));
        }
        Ok(())
    }
}
impl TextPort for Port {
    fn observe(&mut self) -> Result<Observation, DesktopError> {
        self.effect("observe")?;
        Ok(self.observation)
    }
    fn write(&mut self, _document: &DocumentIndex) -> Result<(), DesktopError> {
        self.effect("write")
    }
    fn restore(
        &mut self,
        start: i32,
        end: i32,
        first: i32,
        follow: bool,
    ) -> Result<(), DesktopError> {
        self.effect("restore")?;
        self.restored = Some((start, end, first, follow));
        Ok(())
    }
}
#[test]
fn selected_text_and_scrolled_view_keep_record_anchors_when_new_output_arrives() -> TestResult {
    let (old, _, _) = index(&source(1, &[(1, "gone"), (2, "chosen"), (3, "tail")])?)?;
    let (next, _, _) = index(&source(2, &[(2, "chosen"), (3, "tail"), (4, "new")])?)?;
    let mut port = Port::new(Observation { start: 5, end: 11, first: 5, at_bottom: true });
    replace(Some(&old), &next, &mut port)?;
    assert_eq!(port.restored, Some((0, 6, 0, false)));
    assert_eq!(port.calls, ["observe", "write", "restore"]);
    port.observation = Observation { start: 12, end: 12, first: 5, at_bottom: false };
    replace(Some(&old), &next, &mut port)?;
    assert_eq!(port.restored, Some((7, 7, 0, false)));
    Ok(())
}
#[test]
fn tail_follow_requires_empty_selection_and_actual_bottom_and_never_grants_focus() -> TestResult {
    let (old, _, _) = index(&source(1, &[(1, "first")])?)?;
    let (next, _, _) = index(&source(2, &[(1, "first"), (2, "next")])?)?;
    let mut port = Port::new(Observation { start: 5, end: 5, first: 0, at_bottom: true });
    replace(Some(&old), &next, &mut port)?;
    assert_eq!(port.restored, Some((10, 10, 0, true)));
    assert_eq!(port.calls, ["observe", "write", "restore"]);
    port.calls.clear();
    port.observation.at_bottom = false;
    replace(Some(&old), &next, &mut port)?;
    assert_eq!(port.restored, Some((5, 5, 0, false)));
    Ok(())
}
#[test]
fn foreign_resource_clears_old_selection_and_never_reuses_old_record_authority() -> TestResult {
    let (old, _, _) = index(&source(1, &[(1, "old")])?)?;
    let mut foreign = source(1, &[(1, "different")])?;
    foreign.key.generation = sugarsubstitute_contracts::lifecycle::ResourceGeneration::new(2)?;
    let (next, _, _) = index(&foreign)?;
    let mut port = Port::new(Observation { start: 0, end: 3, first: 0, at_bottom: false });
    replace(Some(&old), &next, &mut port)?;
    assert_eq!(port.restored, Some((9, 9, 0, true)));
    Ok(())
}
#[test]
fn partial_native_failure_keeps_both_exact_document_claims_and_closes_further_publication()
-> TestResult {
    for failure in ["observe", "write", "restore"] {
        let (old, old_frame, old_release) = index(&source(1, &[(1, "old")])?)?;
        let mut documents = Documents::new(old);
        let mut port = Port::new(Observation::default());
        documents.publish(&mut port)?;
        let (next, next_frame, next_release) = index(&source(2, &[(1, "new")])?)?;
        documents.stage(next)?;
        port.fail = Some(failure);
        port.calls.clear();
        let error = documents.publish(&mut port).err().ok_or("native failure lost")?;
        assert!(error.source().and_then(|cause| cause.downcast_ref::<std::io::Error>()).is_some());
        assert_eq!(documents.current().ok_or("old document lost")?.revision, 1);
        assert!(documents.pending());
        let calls = port.calls.clone();
        assert!(documents.publish(&mut port).is_err());
        assert_eq!(port.calls, calls);
        assert!(old_frame.claim_storage(1).is_err());
        assert!(next_frame.claim_storage(1).is_err());
        drop((old_frame, next_frame));
        assert_eq!(old_release.load(Ordering::SeqCst), 0);
        assert_eq!(next_release.load(Ordering::SeqCst), 0);
        drop(documents);
        assert_eq!(old_release.load(Ordering::SeqCst), 1);
        assert_eq!(next_release.load(Ordering::SeqCst), 1);
    }
    Ok(())
}
#[test]
fn successful_replacement_retires_only_old_copy_and_rejects_stale_revision_before_effects()
-> TestResult {
    let (old, frame, released) = index(&source(3, &[(1, "old")])?)?;
    let mut documents = Documents::new(old);
    let mut port = Port::new(Observation::default());
    documents.publish(&mut port)?;
    drop(frame);
    let (stale, _, _) = index(&source(2, &[(1, "stale")])?)?;
    assert!(documents.stage(stale).is_err());
    assert_eq!(documents.current().ok_or("old lost")?.revision, 3);
    assert_eq!(released.load(Ordering::SeqCst), 0);
    let (next, _, _) = index(&source(4, &[(1, "next")])?)?;
    documents.stage(next)?;
    documents.publish(&mut port)?;
    assert_eq!(released.load(Ordering::SeqCst), 1);
    assert!(!documents.pending());
    assert_eq!(documents.current().ok_or("new lost")?.revision, 4);
    Ok(())
}
