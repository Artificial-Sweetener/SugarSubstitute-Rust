// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Test the same bounded modal pump used by actual native hook dispatch.
use super::*;
use std::error::Error;
use sugarsubstitute_contracts::execution::MemoryRelease;
struct Release;
impl MemoryRelease for Release {
    fn release(&self, _: u64) {}
}
unsafe fn idle(_: *mut c_void) -> Result<ReportSurfaceUpdate, ReportSurfaceError> {
    Ok(ReportSurfaceUpdate::None)
}
pub(super) fn state() -> Result<Hook, Box<dyn Error>> {
    let lease = MemoryLease::new(4096, Box::new(Release));
    Ok(Hook {
        context: std::ptr::null_mut(),
        poller: Some(idle),
        active: false,
        initialized: false,
        installed: false,
        destroyed: false,
        timer: 0,
        update: None,
        update_at: None,
        cancel_requested: dispatch::ModalCancellation::fixture(&lease)?,
        cancel_posted: false,
        failure: None,
        save: Text::new("Save", &lease)?,
        cancel_label: Text::new("Cancel", &lease)?,
        consent: Some(consent::Consent::new(
            Text::new("Replace an existing destination", &lease)?,
            &lease,
        )),
    })
}
#[test]
fn report_modal_completion_retains_one_update_and_fences_further_polling()
-> Result<(), Box<dyn Error>> {
    use sugarsubstitute_contracts::{incident::*, lifecycle::*};
    let owner = CoordinatorKey {
        namespace: NamespaceId::new([1; 32]),
        epoch: CoordinatorEpoch::new([2; 16])?,
    };
    let binding = ReportBinding::new(
        IncidentKey::new(IncidentSubject::Supervisor(owner)),
        IncidentRevision::new(2)?,
        ReportSessionId::new(1)?,
    );
    let acknowledgement = DurableAcknowledgement::new(
        binding.incident(),
        IncidentRevision::new(3)?,
        IncidentUtc::from_unix_seconds(10)?,
    );
    let mut state = state()?;
    assert!(state.begin_poll()?.is_some());
    assert!(
        state.complete_poll(Ok(ReportSurfaceUpdate::CloseAfterDurableAck {
            binding,
            acknowledgement
        }))
    );
    assert!(state.begin_poll()?.is_none());
    let Some(ReportSurfaceUpdate::CloseAfterDurableAck {
        binding: actual,
        acknowledgement: actual_ack,
    }) = state.update.take()
    else {
        return Err("lost exact deferred update".into());
    };
    assert_eq!(actual, binding);
    assert_eq!(actual_ack, acknowledgement);
    Ok(())
}
#[test]
fn idle_modal_pump_keeps_original_admission_and_can_observe_later_completion()
-> Result<(), Box<dyn Error>> {
    let mut state = state()?;
    let context = state.context;
    assert!(state.begin_poll()?.is_some());
    assert!(!state.complete_poll(Ok(ReportSurfaceUpdate::None)));
    assert_eq!(state.context, context);
    assert!(state.begin_poll()?.is_some());
    assert!(state.complete_poll(Err(ReportSurfaceError::new(
        "actual content failure",
        std::io::Error::other("original")
    ))));
    assert!(state.begin_poll()?.is_none());
    assert_eq!(
        state.failure.as_ref().ok_or("missing failure")?.operation(),
        "actual content failure"
    );
    Ok(())
}
#[test]
fn recursive_modal_poll_rejects_before_second_content_borrow_and_preserves_first_cause()
-> Result<(), Box<dyn Error>> {
    let mut state = state()?;
    assert!(state.begin_poll()?.is_some());
    let cause = state.begin_poll().err().ok_or("recursive borrow accepted")?;
    assert_eq!(cause.operation(), "report_picker_recursive_poll");
    state.fail(cause);
    state.complete_poll(Err(ReportSurfaceError::rejected("later content failure")));
    assert_eq!(
        state.failure.as_ref().ok_or("lost first cause")?.operation(),
        "report_picker_recursive_poll"
    );
    assert!(state.begin_poll()?.is_none());
    Ok(())
}
#[test]
fn disarmed_native_hook_keeps_unchecked_initialization_custody_without_expired_borrow()
-> Result<(), Box<dyn Error>> {
    let mut state = state()?;
    state.poller = None;
    state.context = std::ptr::null_mut();
    assert!(state.begin_poll()?.is_none());
    assert!(!state.active);
    assert!(state.retired());
    // Actual WM_INITDIALOG may precede a failed native pointer registration.
    state.initialized = true;
    assert!(!state.retired());
    state.destroyed = true;
    assert!(!state.retired());
    Ok(())
}

#[test]
fn report_modal_deferred_close_posts_cancel_once_without_polling_or_retiring_hook()
-> Result<(), Box<dyn Error>> {
    let mut state = state()?;
    assert!(!state.take_cancel());
    state.cancel_requested.set(true);
    assert!(state.take_cancel());
    assert!(!state.take_cancel());
    assert!(!state.active);
    assert!(state.poller.is_some());
    assert!(!state.initialized);
    Ok(())
}
#[test]
fn report_modal_update_observation_keeps_first_actual_close_control_instant()
-> Result<(), Box<dyn Error>> {
    use sugarsubstitute_contracts::{incident::*, lifecycle::*};
    let owner = CoordinatorKey {
        namespace: NamespaceId::new([1; 32]),
        epoch: CoordinatorEpoch::new([2; 16])?,
    };
    let incident = IncidentKey::new(IncidentSubject::Supervisor(owner));
    let binding = ReportBinding::new(incident, IncidentRevision::new(1)?, ReportSessionId::new(1)?);
    let mut state = state()?;
    assert!(state.begin_poll()?.is_some());
    let acknowledgement = DurableAcknowledgement::new(
        incident,
        IncidentRevision::new(2)?,
        IncidentUtc::from_unix_seconds(10)?,
    );
    assert!(
        state.complete_poll(Ok(ReportSurfaceUpdate::CloseAfterDurableAck {
            binding,
            acknowledgement
        }))
    );
    let original = state.update_at.ok_or("actual update observation time absent")?;
    assert!(state.begin_poll()?.is_none());
    assert_eq!(state.update_at, Some(original));
    Ok(())
}
