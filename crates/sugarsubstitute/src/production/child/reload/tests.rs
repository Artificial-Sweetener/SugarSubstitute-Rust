// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Actual child request/control ownership with external native observations and deterministic time.
use super::*;
#[test]
fn reload_requires_current_authorized_main_before_any_command_or_binding_change() -> Result<()> {
    let (mut child, owner, _) = super::super::test_support::child()?;
    let original = child.identity;
    assert!(child.request_reload(original).is_err());
    child.phase = Some(StartupPhase::Running);
    assert!(child.request_reload(original).is_err());
    assert!(child.reload.is_none());
    assert!(child.commands.iter().all(Option::is_none));
    assert_eq!(child.identity, original);
    drop((child, owner));
    Ok(())
}
#[test]
fn actual_main_retired_without_profile_never_manufactures_root_ack_or_new_main() -> Result<()> {
    let (mut child, owner, _) = super::super::test_support::child()?;
    let original = child.identity;
    child.observe(DesktopStartupObservation::MainRetired(original), None)?;
    assert!(child.shutdown.main_retired);
    assert!(!child.shutdown.main_retirement_ack);
    assert_eq!(child.identity, original);
    assert!(child.commands.iter().all(Option::is_none));
    drop((child, owner));
    Ok(())
}
#[test]
fn duplicate_native_reload_preserves_first_retirement_cutoff_and_foreign_identity_has_no_effect()
-> Result<()> {
    let (mut child, owner, clock) = super::super::test_support::child()?;
    let original = child.identity;
    let next =
        MainIdentity { launch: LaunchId::new(2)?, navigation: Generation::new(2)?, ..original };
    let until = Moment::from_nanos(15000000000);
    let lease = LeaseReference {
        resource: ResourceId::new(1)?,
        generation: ResourceGeneration::new(1)?,
        lease: LeaseId::new(1)?,
    };
    // Retained correlation is inert: this fixture supplies no profile, source or backend authority.
    child.reload = Some(FrontendReload {
        previous: original,
        next,
        lease,
        until,
        stage: Stage::Retiring,
        origin: None,
    });
    clock.0.store(14000000000, std::sync::atomic::Ordering::SeqCst);
    child.request_reload(original)?;
    assert_eq!(child.reload.as_ref().ok_or("Reload correlation lost")?.until(), until);
    assert!(child.request_reload(next).is_err());
    assert_eq!(child.identity, original);
    assert!(child.commands.iter().all(Option::is_none));
    drop((child, owner));
    Ok(())
}

#[test]
fn reload_wait_lifts_only_after_admission_and_never_renews_retirement_or_wire_cutoff() -> Result<()>
{
    let (child, owner, _) = super::super::test_support::child()?;
    let identity = child.identity;
    let lease = LeaseReference {
        resource: ResourceId::new(1)?,
        generation: ResourceGeneration::new(1)?,
        lease: LeaseId::new(1)?,
    };
    let until = Moment::from_nanos(30_000_000_000);
    let header = WireHeader {
        run: identity.run,
        connection: Some(ConnectionId::new(1)?),
        request: Some(RequestId::new(1)?),
        deadline_qpc: 300,
    };
    let mut reload = FrontendReload {
        previous: identity,
        next: identity,
        lease,
        until,
        origin: None,
        stage: Stage::Retiring,
    };
    assert_eq!(reload.deadline(), Some(until));
    reload.stage = Stage::Request { header, admitted: false };
    assert_eq!(reload.deadline(), Some(until));
    reload.stage = Stage::Request { header, admitted: true };
    assert_eq!(reload.deadline(), None);
    assert_eq!(reload.until(), until);
    assert!(
        matches!(reload.stage, Stage::Request { header: original, admitted: true } if original == header)
    );
    reload.stage = Stage::Granted;
    assert_eq!(reload.deadline(), None);
    drop((child, owner));
    Ok(())
}
