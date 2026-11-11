// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise actual download admission/custody with deterministic native boundary outcomes.

use super::*;
use std::{error::Error, rc::Rc};

type Result = std::result::Result<(), Box<dyn Error>>;

#[derive(Clone)]
struct Operation(Rc<Facts>);
struct Facts {
    state: Cell<DownloadState>,
    state_error: Cell<bool>,
    cancel_error: Cell<bool>,
    cancels: Cell<u32>,
    observe: RefCell<Option<Box<dyn FnOnce()>>>,
    cancel_observe: RefCell<Option<Box<dyn FnOnce()>>>,
}

impl Operation {
    /// Create a controllable external operation; cancellation does not fabricate completion.
    fn new(state: DownloadState) -> Self {
        Self(Rc::new(Facts {
            state: Cell::new(state),
            state_error: Cell::new(false),
            cancel_error: Cell::new(false),
            cancels: Cell::new(0),
            observe: RefCell::new(None),
            cancel_observe: RefCell::new(None),
        }))
    }
}
impl DownloadOperation for Operation {
    fn state(&self) -> std::result::Result<DownloadState, DesktopError> {
        let observed = self.0.observe.borrow_mut().take();
        if let Some(observed) = observed {
            observed();
        }
        if self.0.state_error.get() {
            return Err(DesktopError::message("test_download_state", "native state failure"));
        }
        Ok(self.0.state.get())
    }
    fn cancel(&self) -> std::result::Result<(), DesktopError> {
        let observed = self.0.cancel_observe.borrow_mut().take();
        if let Some(observed) = observed {
            observed();
        }
        self.0.cancels.set(self.0.cancels.get() + 1);
        if self.0.cancel_error.get() {
            return Err(DesktopError::message("test_download_cancel", "native cancel failure"));
        }
        Ok(())
    }
}

#[test]
fn served_workflow_json_output_png_and_frontend_blobs_use_the_exact_origin() -> Result {
    for root in ["http://127.0.0.1:8188/", "http://[::1]:8188/", "http://localhost:8188/"] {
        let origin = LoopbackOrigin::parse(root, 8188)?;
        for suffix in ["workflow.json", "view?filename=result.png&type=output"] {
            assert!(admits_download(origin, true, true, root, &format!("{root}{suffix}")));
        }
        assert!(admits_download(
            origin,
            true,
            true,
            root,
            &format!("blob:{root}7aa068a3-24af-43ac-b8ef-85d4226ac611")
        ));
    }
    Ok(())
}

#[test]
fn downloads_require_actual_reveal_open_admission_and_current_trusted_document() -> Result {
    let root = "http://127.0.0.1:8188/";
    let origin = LoopbackOrigin::parse(root, 8188)?;
    let uri = format!("blob:{root}7aa068a3-24af-43ac-b8ef-85d4226ac611");
    for (revealed, accepting, source) in [
        (false, true, root),
        (true, false, root),
        (true, true, "about:blank"),
        (true, true, "http://127.0.0.1:8189/"),
        (true, true, "http://example.com/"),
    ] {
        assert!(!admits_download(origin, revealed, accepting, source, &uri));
    }
    Ok(())
}

#[test]
fn foreign_opaque_local_file_and_normalized_download_authorities_are_rejected() -> Result {
    let root = "http://127.0.0.1:8188/";
    let origin = LoopbackOrigin::parse(root, 8188)?;
    for uri in [
        "https://127.0.0.1:8188/workflow.json",
        "http://127.0.0.1:8189/result.png",
        "http://localhost:8188/result.png",
        "http://127.1:8188/result.png",
        "http://127.0.0.1:08188/result.png",
        "http://user@127.0.0.1:8188/a",
        "http://127.0.0.1:8188@external/a",
        "blob:http://example.com/id",
        "blob:null/id",
        "blob:blob:http://127.0.0.1:8188/id",
        "data:application/json,{}",
        "file:///D:/private.txt",
        "javascript:alert(1)",
        "http://127.0.0.1:8188/\0",
    ] {
        assert!(!admits_download(origin, true, true, root, uri), "Unadmitted URI: {uri}");
    }
    let large = format!("blob:{root}{}", "x".repeat(crate::main_navigation::MAX_URL_BYTES));
    assert!(!admits_download(origin, true, true, root, &large));
    Ok(())
}

#[test]
fn bounded_inventory_rejects_overload_and_recovers_only_observed_terminal_capacity() -> Result {
    let owner = Downloads::new();
    let active = Operation::new(DownloadState::Retained);
    for _ in 0..MAX_DOWNLOADS {
        assert!(owner.retain(active.clone())?);
    }
    assert!(!owner.retain(active.clone())?);
    owner.reap_terminal()?;
    assert!(!owner.retain(active.clone())?);
    active.0.state.set(DownloadState::Terminal);
    owner.reap_terminal()?;
    for _ in 0..MAX_DOWNLOADS {
        assert!(owner.retain(Operation::new(DownloadState::Retained))?);
    }
    assert!(!owner.retain(active)?);
    Ok(())
}

#[test]
fn cancellation_is_once_only_and_needs_an_independent_terminal_observation() -> Result {
    let owner = Downloads::new();
    let operation = Operation::new(DownloadState::Retained);
    assert!(owner.retain(operation.clone())?);
    assert!(!owner.retire_step()?);
    assert_eq!(operation.0.cancels.get(), 1);
    assert!(!owner.retire_step()?);
    assert_eq!(operation.0.cancels.get(), 1);
    assert!(!owner.retain(Operation::new(DownloadState::Retained))?);
    operation.0.state.set(DownloadState::Terminal);
    assert!(!owner.retire_step()?);
    assert!(owner.retire_step()?);
    assert_eq!(operation.0.cancels.get(), 1);
    Ok(())
}

#[test]
fn already_completed_downloads_are_not_cancelled_during_retirement() -> Result {
    let owner = Downloads::new();
    let operation = Operation::new(DownloadState::Terminal);
    assert!(owner.retain(operation.clone())?);
    assert!(!owner.retire_step()?);
    assert!(owner.retire_step()?);
    assert_eq!(operation.0.cancels.get(), 0);
    Ok(())
}

#[test]
fn failed_native_cancel_preserves_custody_without_reissuing_or_inventing_terminal_state() -> Result
{
    let owner = Downloads::new();
    let operation = Operation::new(DownloadState::Retained);
    operation.0.cancel_error.set(true);
    assert!(owner.retain(operation.clone())?);
    let error = owner.retire_step().err().ok_or("Cancellation failure was discarded")?;
    assert!(error.to_string().contains("native cancel failure"));
    assert_eq!(operation.0.cancels.get(), 1);
    assert!(!owner.retire_step()?);
    assert_eq!(operation.0.cancels.get(), 1);
    operation.0.state.set(DownloadState::Terminal);
    assert!(!owner.retire_step()?);
    assert!(owner.retire_step()?);
    Ok(())
}

#[test]
fn failed_native_state_cannot_release_operation_or_issue_cancel() -> Result {
    let owner = Downloads::new();
    let operation = Operation::new(DownloadState::Retained);
    operation.0.state_error.set(true);
    for _ in 0..MAX_DOWNLOADS {
        assert!(owner.retain(operation.clone())?);
    }
    assert!(owner.reap_terminal().is_err());
    assert!(!owner.retain(operation.clone())?);
    let error = owner.retire_step().err().ok_or("Observation failure was discarded")?;
    assert!(error.to_string().contains("native state failure"));
    assert_eq!(operation.0.cancels.get(), 0);
    assert!(!owner.retain(Operation::new(DownloadState::Retained))?);
    Ok(())
}

#[test]
fn reentrant_native_observation_cannot_remove_a_replacement_download_identity() -> Result {
    let owner = Rc::new(Downloads::new());
    let old = Operation::new(DownloadState::Terminal);
    let replacement = Operation::new(DownloadState::Retained);
    assert!(owner.retain(old.clone())?);
    let captured = owner.clone();
    let next = replacement.clone();
    *old.0.observe.borrow_mut() = Some(Box::new(move || {
        assert!(captured.reap_terminal().is_ok());
        assert_eq!(captured.retain(next).ok(), Some(true));
    }));
    owner.reap_terminal()?;
    for _ in 1..MAX_DOWNLOADS {
        assert!(owner.retain(Operation::new(DownloadState::Retained))?);
    }
    assert!(!owner.retain(Operation::new(DownloadState::Retained))?);
    assert!(!owner.retire_step()?);
    assert_eq!(replacement.0.cancels.get(), 1);
    Ok(())
}

#[test]
fn empty_retirement_irreversibly_closes_admission() -> Result {
    let owner = Downloads::new();
    assert!(owner.retire_step()?);
    assert!(!owner.retain(Operation::new(DownloadState::Retained))?);
    assert!(owner.retire_step()?);
    Ok(())
}

#[test]
fn reentrant_native_observation_cannot_issue_the_same_cancellation_twice() -> Result {
    let owner = Rc::new(Downloads::new());
    let operation = Operation::new(DownloadState::Retained);
    assert!(owner.retain(operation.clone())?);
    let captured = owner.clone();
    *operation.0.observe.borrow_mut() = Some(Box::new(move || {
        assert_eq!(captured.retire_step().ok(), Some(false));
    }));
    assert!(!owner.retire_step()?);
    assert_eq!(operation.0.cancels.get(), 1);
    assert!(!owner.retire_step()?);
    assert_eq!(operation.0.cancels.get(), 1);
    Ok(())
}

#[test]
fn one_pending_cancellation_cannot_prevent_other_admitted_downloads_being_cancelled() -> Result {
    let owner = Downloads::new();
    let operations: [Operation; MAX_DOWNLOADS] =
        std::array::from_fn(|_| Operation::new(DownloadState::Retained));
    for operation in &operations {
        assert!(owner.retain(operation.clone())?);
    }
    for _ in 0..MAX_DOWNLOADS {
        assert!(!owner.retire_step()?);
    }
    for operation in &operations {
        assert_eq!(operation.0.cancels.get(), 1);
    }
    assert!(!owner.retire_step()?);
    for operation in &operations {
        assert_eq!(operation.0.cancels.get(), 1);
        operation.0.state.set(DownloadState::Terminal);
    }
    for _ in 0..MAX_DOWNLOADS {
        assert!(!owner.retire_step()?);
    }
    assert!(owner.retire_step()?);
    Ok(())
}

#[path = "main_downloads/failure_tests.rs"]
mod failure_tests;
