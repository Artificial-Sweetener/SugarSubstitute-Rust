// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise actual bounded retirement and failed callback admission with external native outcomes.
use super::*;

fn setter_failure() -> DesktopError {
    DesktopError::new("cancel_unadmitted_download", std::io::Error::from_raw_os_error(5))
}
fn partial(error: &DesktopError) -> std::result::Result<&StartingFailure, Box<dyn Error>> {
    error
        .source()
        .and_then(|cause| cause.downcast_ref::<StartingFailure>())
        .ok_or_else(|| "Original partial cancellation causes lost".into())
}

#[test]
fn failed_state_getter_cannot_starve_another_original_download_cancel() -> Result {
    let owner = Downloads::new();
    let first = Operation::new(DownloadState::Retained);
    first.0.state_error.set(true);
    let second = Operation::new(DownloadState::Retained);
    assert!(owner.retain(first.clone())?);
    assert!(owner.retain(second.clone())?);
    let identity = owner.slots.borrow()[0].as_ref().map(|slot| slot.identity);
    assert!(owner.retire_step().is_err());
    assert_eq!(first.0.cancels.get(), 0);
    assert!(!owner.retire_step()?);
    assert_eq!(second.0.cancels.get(), 1);
    assert_eq!(owner.slots.borrow()[0].as_ref().map(|slot| slot.identity), identity);
    assert!(owner.retire_step().is_err());
    assert!(!owner.retire_step()?);
    assert_eq!(second.0.cancels.get(), 1);
    Ok(())
}

#[test]
fn failed_event_cancellation_retains_exact_operation_before_once_only_cancel() -> Result {
    let owner = Rc::new(Downloads::<Operation>::new());
    let operation = Operation::new(DownloadState::Retained);
    let saw_retained = Rc::new(Cell::new(false));
    let observed = saw_retained.clone();
    let weak_owner = Rc::downgrade(&owner);
    let weak_operation = Rc::downgrade(&operation.0);
    *operation.0.cancel_observe.borrow_mut() = Some(Box::new(move || {
        if let (Some(owner), Some(exact)) = (weak_owner.upgrade(), weak_operation.upgrade()) {
            observed.set(
                owner
                    .slots
                    .borrow()
                    .iter()
                    .flatten()
                    .any(|slot| Rc::ptr_eq(&slot.operation.0, &exact) && slot.cancel_requested),
            );
        }
    }));
    let first = setter_failure();
    let first_pointer = first.source().map(std::ptr::from_ref);
    let getters = Cell::new(0);
    let result = owner.cancel_unadmitted(
        || Err(first),
        || {
            getters.set(getters.get() + 1);
            (Some(operation.clone()), Ok(()))
        },
    );
    let error = result.err().ok_or("Failed native setter was hidden")?;
    assert_eq!(getters.get(), 1);
    assert!(saw_retained.get());
    assert_eq!(operation.0.cancels.get(), 1);
    assert!(owner.closing.get());
    let retained = owner.slots.borrow()[0]
        .as_ref()
        .map(|slot| (slot.operation.clone(), slot.cancel_requested));
    let (retained, requested) = retained.ok_or("Exact partial output was not retained")?;
    assert!(Rc::ptr_eq(&retained.0, &operation.0));
    assert!(requested);
    assert_eq!(partial(&error)?.first.source().map(std::ptr::from_ref), first_pointer);
    assert!(!owner.retire_step()?);
    assert_eq!(operation.0.cancels.get(), 1);
    operation.0.state.set(DownloadState::Terminal);
    assert!(!owner.retire_step()?);
    assert!(owner.retire_step()?);
    assert_eq!(partial(&error)?.first.source().map(std::ptr::from_ref), first_pointer);
    Ok(())
}

#[test]
fn full_eight_slots_preserve_uncertain_ninth_output_and_progress_all_original_cancels() -> Result {
    let owner = Downloads::new();
    let originals: [Operation; MAX_DOWNLOADS] =
        std::array::from_fn(|_| Operation::new(DownloadState::Retained));
    for operation in &originals {
        assert!(owner.retain(operation.clone())?);
    }
    let returned = Operation::new(DownloadState::Retained);
    let error = owner
        .cancel_unadmitted(|| Err(setter_failure()), || (Some(returned.clone()), Ok(())))
        .err()
        .ok_or("Full-capacity cancellation failure disappeared")?;
    assert_eq!(returned.0.cancels.get(), 1);
    assert_eq!(owner.slots.borrow().iter().flatten().count(), MAX_DOWNLOADS);
    assert!(owner.uncertain.get());
    for _ in 0..MAX_DOWNLOADS {
        assert!(!owner.retire_step()?);
    }
    for operation in &originals {
        assert_eq!(operation.0.cancels.get(), 1);
        operation.0.state.set(DownloadState::Terminal);
    }
    for _ in 0..MAX_DOWNLOADS {
        assert!(!owner.retire_step()?);
    }
    assert_eq!(owner.slots.borrow().iter().flatten().count(), 0);
    assert!(!owner.retire_step()?);
    assert_eq!(returned.0.cancels.get(), 1);
    assert!(partial(&error)?.admission.is_none());
    assert!(!owner.retain(Operation::new(DownloadState::Retained))?);
    Ok(())
}

#[test]
fn failed_getter_nonnull_output_and_cancel_error_retain_all_original_causes() -> Result {
    let owner = Downloads::new();
    let operation = Operation::new(DownloadState::Retained);
    operation.0.cancel_error.set(true);
    let first = setter_failure();
    let pointer = first.source().map(std::ptr::from_ref);
    let getter =
        DesktopError::message("get_browser_download", "getter failed after partial output");
    let getter_pointer = getter.source().map(std::ptr::from_ref);
    let error = owner
        .cancel_unadmitted(|| Err(first), || (Some(operation.clone()), Err(getter)))
        .err()
        .ok_or("Partial getter failure disappeared")?;
    let failed = partial(&error)?;
    assert_eq!(failed.first.source().map(std::ptr::from_ref), pointer);
    assert_eq!(
        failed.getter.as_ref().and_then(Error::source).map(std::ptr::from_ref),
        getter_pointer
    );
    assert!(failed.cancel.is_some());
    assert_eq!(operation.0.cancels.get(), 1);
    assert_eq!(owner.slots.borrow().iter().flatten().count(), 1);
    assert!(owner.uncertain.get());
    assert!(!owner.retire_step()?);
    assert_eq!(operation.0.cancels.get(), 1);
    operation.0.state.set(DownloadState::Terminal);
    assert!(!owner.retire_step()?);
    assert!(!owner.retire_step()?);
    Ok(())
}

#[test]
fn missing_partial_operation_cannot_make_empty_inventory_a_retirement_proof() -> Result {
    let owner = Downloads::<Operation>::new();
    let error = owner
        .cancel_unadmitted(
            || Err(setter_failure()),
            || (None, Err(DesktopError::message("get_browser_download", "getter failed"))),
        )
        .err()
        .ok_or("Missing-output native failure disappeared")?;
    assert!(partial(&error)?.getter.is_some());
    assert_eq!(owner.slots.borrow().iter().flatten().count(), 0);
    assert!(!owner.retire_step()?);
    assert!(!owner.retire_step()?);
    let absent_event = Downloads::<Operation>::new();
    absent_event.retain_unknown_native_effect();
    assert!(!absent_event.retire_step()?);
    Ok(())
}

#[test]
fn successful_event_cancellation_does_not_query_operation_or_close_normal_admission() -> Result {
    let owner = Downloads::<Operation>::new();
    let getters = Cell::new(0);
    owner.cancel_unadmitted(
        || Ok(()),
        || {
            getters.set(getters.get() + 1);
            (None, Ok(()))
        },
    )?;
    assert_eq!(getters.get(), 0);
    assert!(!owner.closing.get());
    for _ in 0..MAX_DOWNLOADS {
        assert!(owner.retain(Operation::new(DownloadState::Retained))?);
    }
    assert!(!owner.retain(Operation::new(DownloadState::Retained))?);
    Ok(())
}
