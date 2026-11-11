// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Native-free tests of the actual charged owner, exclusive loan and fixed deferred facts.
use super::*;
use crate::report::{
    NativeReportView, ReportRequestId, ReportSurfaceObservation, ReportSurfaceUpdate,
};
use std::{error::Error, num::NonZeroUsize};
use sugarsubstitute_contracts::{
    execution::{CompletionWake, MemoryLease, MemoryRelease},
    incident::*,
    lifecycle::*,
};
struct Release;
impl MemoryRelease for Release {
    fn release(&self, _: u64) {}
}
#[derive(Default)]
struct Content {
    polls: usize,
    shutdowns: usize,
    drop_probe: Option<DropProbe>,
}
impl Drop for Content {
    fn drop(&mut self) {
        if let Some(probe) = &self.drop_probe {
            probe.charged.set(probe.lease.claim(probe.lease.bytes()).is_err());
            probe.dropped.set(true);
        }
    }
}
impl ReportSessionContent for Content {
    fn start(&mut self, _: Arc<dyn CompletionWake>) -> Result<(), ReportSurfaceError> {
        Ok(())
    }
    fn poll(&mut self, _: NonZeroUsize) -> Result<ReportSurfaceUpdate, ReportSurfaceError> {
        self.polls += 1;
        Ok(ReportSurfaceUpdate::None)
    }
    fn action(&mut self, _: ReportRequestId, _: ReportCommand) -> Result<(), ReportSurfaceError> {
        Ok(())
    }
    fn observe(&mut self, _: ReportSurfaceObservation) -> Result<(), ReportSurfaceError> {
        Ok(())
    }
    fn request_shutdown(&mut self) -> Result<(), ReportSurfaceError> {
        self.shutdowns += 1;
        Ok(())
    }
    fn poll_shutdown(&mut self) -> Result<bool, ReportSurfaceError> {
        Ok(false)
    }
    fn finish_shutdown(&mut self) -> Result<(), ReportSurfaceError> {
        Ok(())
    }
}
fn options(lease: &MemoryLease) -> Result<ReportSurfaceOptions, Box<dyn Error>> {
    let owner = CoordinatorKey {
        namespace: NamespaceId::new([1; 32]),
        epoch: CoordinatorEpoch::new([2; 16])?,
    };
    let binding = ReportBinding::new(
        IncidentKey::new(IncidentSubject::Supervisor(owner)),
        IncidentRevision::new(1)?,
        ReportSessionId::new(1)?,
    );
    let view = NativeReportView::from_claimed(
        binding,
        "Report\n".into(),
        lease.claim(512)?,
        ReportRestartAvailability::Available,
    )?;
    Ok(ReportSurfaceOptions::new(view, "en", false, 960, 740, lease.clone())?)
}
/// Observe the actual content destructor against its original allocation ledger.
struct DropProbe {
    lease: MemoryLease,
    dropped: Rc<Cell<bool>>,
    charged: Rc<Cell<bool>>,
}
/// One isolated charged owner and its original ledger.
struct Fixture {
    owner: Rc<SurfaceOwner<Content>>,
    lease: MemoryLease,
}
fn fixture() -> Result<Fixture, Box<dyn Error>> {
    let lease = MemoryLease::new(ReportSurfaceOptions::KNOWN_STORAGE_BYTES, Box::new(Release));
    super::super::validate_content_capacity::<Content>()?;
    let (session, storage) = session::Session::new(options(&lease)?);
    Ok(Fixture { owner: SurfaceOwner::new(session, Content::default(), storage), lease })
}
#[test]
fn report_dispatch_loan_excludes_nested_content_and_restores_exact_box_and_real_claim()
-> Result<(), Box<dyn Error>> {
    let Fixture { owner, lease } = fixture()?;
    let pointer;
    {
        let mut loan = owner.loan().ok_or("actual first loan absent")?;
        let state = loan.state()?;
        pointer = std::ptr::from_mut(state);
        assert_eq!(state._funding._storage.bytes(), ReportSurfaceOptions::DISPATCH_STORAGE_BYTES);
        assert!(owner.loan().is_none());
        owner.message_turn();
        assert_eq!(state.content.polls, 0);
        assert!(lease.claim(lease.bytes()).is_err());
    }
    {
        let mut loan = owner.loan().ok_or("actual owner not restored")?;
        assert_eq!(std::ptr::from_mut(loan.state()?), pointer);
    }
    // This pure fixture has no native registrations; explicit transfer releases only its actual data.
    let state = owner.take_final()?;
    drop(owner);
    assert!(lease.claim(lease.bytes()).is_err());
    drop(state);
    let full = lease.claim(lease.bytes())?;
    drop(full);
    Ok(())
}
#[test]
fn report_dispatch_capacity_refuses_oversized_metadata_and_options_credit_before_effects()
-> Result<(), Box<dyn Error>> {
    assert!(super::super::validate_content_capacity::<[u8; 65536]>().is_err());
    super::super::validate_content_capacity::<Content>()?;
    let lease = MemoryLease::new(ReportSurfaceOptions::KNOWN_STORAGE_BYTES, Box::new(Release));
    let held = lease.claim(lease.bytes() - 512)?;
    assert!(options(&lease).is_err());
    drop(held);
    let full = lease.claim(lease.bytes())?;
    drop(full);
    Ok(())
}
#[test]
fn report_dispatch_deferred_facts_require_own_window_and_keep_first_close_observation()
-> Result<(), Box<dyn Error>> {
    let lease = MemoryLease::new(4096, Box::new(Release));
    let funding = Rc::new(DispatchFunding { _storage: lease.claim(256)? });
    let deferred = Deferred::new(funding);
    // SAFETY: The dummy ID is only compared by authored pure code, never passed to a Tao/native API.
    let window_id = unsafe { tao::window::WindowId::dummy() };
    let close = WindowEvent::CloseRequested;
    deferred.capture(window_id, &close);
    assert!(deferred.close.get().is_none());
    assert!(!deferred.cancel.get());
    deferred.bind(Some(window_id));
    deferred.capture(window_id, &close);
    let original = deferred.close.get();
    deferred.capture(window_id, &close);
    assert!(original.is_some());
    assert_eq!(deferred.close.get(), original);
    assert!(deferred.cancel.get());
    deferred.capture(window_id, &WindowEvent::Resized(tao::dpi::PhysicalSize::new(800, 600)));
    deferred.capture(window_id, &WindowEvent::Destroyed);
    assert!(deferred.layout.get());
    assert!(deferred.destroyed.get().is_some());
    assert_eq!(deferred.close.get(), original);
    Ok(())
}
#[test]
fn report_dispatch_prepared_save_is_one_actual_request_and_close_keeps_original_deadline()
-> Result<(), Box<dyn Error>> {
    let Fixture { owner, lease: _lease } = fixture()?;
    {
        let mut loan = owner.loan().ok_or("actual owner absent")?;
        let state = loan.state()?;
        state.session.state.presented()?;
        let old = state.session.state.binding();
        let binding = ReportBinding::new(old.incident(), IncidentRevision::new(2)?, old.session());
        state.session.state.preflight_replace(old, binding)?;
        state.session.state.replaced(binding, ReportRestartAvailability::Available);
        let selection = ExportSelection::TextAndNativeDump;
        let request = state
            .session
            .state
            .request(binding, super::super::super::state::Intent::Export(selection))?;
        state.session.prepare_save(request, selection)?;
        assert!(state.session.prepare_save(request, selection).is_err());
        let retained = state.session.prepared_save.as_ref().ok_or("admitted Save lost")?;
        assert_eq!(retained.request.id, request.id);
        assert_eq!(retained.request.binding, binding);
        assert_eq!(retained.selection, selection);
        assert_eq!(state.content.polls, 0);
        assert!(state.session.picker.is_none());
        let original = Instant::now();
        state.session.close_at(&mut state.content, original);
        state.session.close_at(&mut state.content, original + std::time::Duration::from_secs(100));
        assert_eq!(
            state.session.close_deadline,
            original.checked_add(std::time::Duration::from_secs(15))
        );
        assert_eq!(state.content.shutdowns, 1);
        assert!(state.session.prepared_save.is_none());
        assert!(!state.session.teardown_ready());
    }
    drop(owner.take_final()?);
    Ok(())
}
#[test]
fn report_dispatch_unwind_returns_original_native_free_owner_and_keeps_its_charge()
-> Result<(), Box<dyn Error>> {
    let Fixture { owner, lease } = fixture()?;
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _loan = owner.loan();
        std::panic::resume_unwind(Box::new("external content boundary fixture"));
    }));
    assert!(result.is_err());
    assert!(lease.claim(lease.bytes()).is_err());
    let mut loan = owner.loan().ok_or("owner lost during unwind")?;
    assert_eq!(loan.state()?.content.polls, 0);
    drop(loan);
    drop(owner.take_final()?);
    drop(owner);
    let full = lease.claim(lease.bytes())?;
    drop(full);
    Ok(())
}

#[test]
fn report_dispatch_real_claim_outlives_content_outer_owner_and_cancel_allocation()
-> Result<(), Box<dyn Error>> {
    for owner_first in [false, true] {
        let Fixture { owner, lease } = fixture()?;
        let dropped = Rc::new(Cell::new(false));
        let charged = Rc::new(Cell::new(false));
        {
            let mut loan = owner.loan().ok_or("owner absent")?;
            loan.state()?.content.drop_probe = Some(DropProbe {
                lease: lease.clone(),
                dropped: dropped.clone(),
                charged: charged.clone(),
            });
        }
        let cancel = owner.deferred.cancel.clone();
        let weak = Rc::downgrade(&owner);
        let state = owner.take_final()?;
        if owner_first {
            drop(owner);
            assert!(weak.upgrade().is_none());
            assert!(!dropped.get());
            drop(state);
        } else {
            drop(state);
            assert!(dropped.get());
            assert!(lease.claim(lease.bytes()).is_err());
            drop(owner);
            assert!(weak.upgrade().is_none());
        }
        assert!(dropped.get() && charged.get());
        assert!(lease.claim(lease.bytes()).is_err());
        drop(cancel);
        let full = lease.claim(lease.bytes())?;
        drop(full);
    }
    Ok(())
}
fn escaping_finish(owner: Rc<SurfaceOwner<Content>>) -> Result<ReportSurfaceError, Box<dyn Error>> {
    let state = owner.take_final()?;
    let mut failure = None;
    owner.finish_content(
        state.content,
        || {
            Err(crate::DesktopError::new(
                "native_free_finisher_boundary",
                std::io::Error::other("Original finalization cause"),
            ))
        },
        &mut failure,
    );
    let cause = failure.ok_or("actual finisher failure absent")?;
    Ok(owner.fund_failure(cause.into()))
}
#[test]
fn report_dispatch_actual_escaping_finisher_error_pins_original_charge_after_owner_disposal()
-> Result<(), Box<dyn Error>> {
    let Fixture { owner, lease } = fixture()?;
    let dropped = Rc::new(Cell::new(false));
    let charged = Rc::new(Cell::new(false));
    {
        let mut loan = owner.loan().ok_or("owner absent")?;
        loan.state()?.content.drop_probe = Some(DropProbe {
            lease: lease.clone(),
            dropped: dropped.clone(),
            charged: charged.clone(),
        });
    }
    let weak = Rc::downgrade(&owner);
    let cause = escaping_finish(owner)?;
    assert!(weak.upgrade().is_none());
    assert!(dropped.get() && charged.get());
    assert!(lease.claim(lease.bytes()).is_err());
    let mut source = Some(&cause as &(dyn Error + 'static));
    let mut original = false;
    for _ in 0..16 {
        let Some(cause) = source else { break };
        original |= cause
            .downcast_ref::<std::io::Error>()
            .is_some_and(|cause| cause.to_string() == "Original finalization cause");
        source = cause.source();
    }
    assert!(original);
    drop(cause);
    let full = lease.claim(lease.bytes())?;
    drop(full);
    Ok(())
}
