// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Pin deterministic capture target identity without granting native ownership.
use super::*;
use crate::{
    backend_custody::{BackendCustodyKey, BackendCustodySession},
    lifecycle::{InstanceId, NamespaceId, RunGeneration},
};
type TestResult = Result<(), Box<dyn std::error::Error>>;
fn owner() -> Result<CoordinatorKey, IdentityError> {
    Ok(CoordinatorKey {
        namespace: NamespaceId::new([1; 32]),
        epoch: CoordinatorEpoch::new([2; 16])?,
    })
}
#[test]
fn backend_capture_subject_has_stable_domain_separation_and_retains_actual_namespace() -> TestResult
{
    let target = CaptureTarget::BackendCustody(BackendCustodyKey::new(
        owner()?,
        BackendCustodySession::new(9)?,
    ));
    let IncidentSubject::Supervisor(subject) = target.capture_subject()? else {
        return Err("Wrong helper subject".into());
    };
    assert_eq!(subject.namespace, owner()?.namespace);
    assert_eq!(
        subject.epoch.bytes(),
        &[175, 244, 231, 91, 80, 66, 211, 124, 93, 246, 99, 210, 250, 235, 91, 114]
    );
    assert_ne!(subject.epoch, owner()?.epoch);
    assert_eq!(target.owner(), owner()?);
    assert!(target.desktop().is_none());
    assert_eq!(target.backend_custody().map(|key| key.session().get()), Some(9));
    Ok(())
}
#[test]
fn backend_owner_namespace_epoch_and_session_each_change_the_capture_subject() -> TestResult {
    let original = BackendCustodyKey::new(owner()?, BackendCustodySession::new(9)?);
    let first = CaptureTarget::BackendCustody(original).capture_subject()?;
    for key in [
        BackendCustodyKey::new(owner()?, BackendCustodySession::new(10)?),
        BackendCustodyKey::new(
            CoordinatorKey { namespace: NamespaceId::new([3; 32]), ..owner()? },
            original.session(),
        ),
        BackendCustodyKey::new(
            CoordinatorKey { epoch: CoordinatorEpoch::new([4; 16])?, ..owner()? },
            original.session(),
        ),
    ] {
        assert_ne!(CaptureTarget::BackendCustody(key).capture_subject()?, first);
    }
    Ok(())
}
#[test]
fn desktop_run_annotation_and_backend_matching_identity_remain_distinct() -> TestResult {
    let run = RunKey {
        owner: owner()?,
        instance: InstanceId::new(9)?,
        generation: RunGeneration::new(1)?,
    };
    let target = CaptureTarget::Desktop(run);
    assert_eq!(target.capture_subject()?, IncidentSubject::Desktop(run));
    assert_eq!(target.desktop(), Some(run));
    assert!(target.backend_custody().is_none());
    assert_ne!(
        target,
        CaptureTarget::BackendCustody(BackendCustodyKey::new(
            owner()?,
            BackendCustodySession::new(9)?
        ))
    );
    Ok(())
}
