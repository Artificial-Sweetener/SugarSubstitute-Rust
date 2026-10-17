// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise passive qualification routing/disposal without forged source or filesystem authority.
use super::*;
use crate::production::qualification::QualificationIntent;
use sugarsubstitute_contracts::{execution::Generation, lifecycle::*, startup::LaunchId};
type TestResult = Result<(), Box<dyn Error>>;
fn identity() -> TestResultIdentity {
    Ok(MainIdentity {
        run: RunKey {
            owner: CoordinatorKey {
                namespace: NamespaceId::new([1; 32]),
                epoch: CoordinatorEpoch::new([2; 16])?,
            },
            instance: InstanceId::new(1)?,
            generation: RunGeneration::new(1)?,
        },
        launch: LaunchId::new(1)?,
        navigation: Generation::new(1)?,
    })
}
type TestResultIdentity = Result<MainIdentity, Box<dyn Error>>;
#[test]
fn ordinary_observation_remains_owned_and_unqualified_close_samples_nothing() -> TestResult {
    let mut owner = ChildQualification::new(QualificationIntent::default());
    let mut observation = Some(DesktopStartupObservation::MainAbsent(identity()?));
    assert!(!owner.retain_unarmed_preview(&mut observation));
    assert!(matches!(observation, Some(DesktopStartupObservation::MainAbsent(_))));
    assert!(!owner.preview_pending());
    assert!(!owner.preview_needs_poll());
    owner.close_preview();
    assert!(!owner.preview_needs_poll());
    Ok(())
}
#[test]
fn passive_rejected_kind_has_one_slot_explicit_excess_disposal_and_observed_close() -> TestResult {
    let mut owner = ChildQualification::new(QualificationIntent::default());
    let main = identity()?;
    let failed = || -> Result<DesktopStartupObservation, Box<dyn Error>> {
        Ok(DesktopStartupObservation::MainPreviewFailed {
            identity: main,
            cause: crate::production::qualification::error_fixture::native_failure()?,
        })
    };
    let mut first = Some(failed()?);
    assert!(owner.retain_unarmed_preview(&mut first));
    assert!(first.is_none());
    assert!(owner.preview_pending());
    assert!(owner.preview_needs_poll());
    let retained = owner.preview.rejected.as_ref().ok_or("Missing bounded rejected slot")?;
    assert_eq!(preview_kind(retained), "failed");
    let mut excess = Some(failed()?);
    assert!(owner.retain_unarmed_preview(&mut excess));
    assert!(excess.is_none());
    assert!(owner.preview.rejected.is_some());
    owner.close_preview();
    assert!(owner.preview_pending());
    assert!(owner.preview_needs_poll());
    owner.poll_preview(Moment::from_nanos(17));
    assert!(!owner.preview_pending());
    Ok(())
}
#[test]
fn full_qualifier_layout_includes_fault_hook_and_preview_before_path_declaration() {
    assert!(size_of::<ChildQualification>() >= Preview::storage_bytes());
    let (internal, original) = sugarsubstitute_execution::desktop_capture_control_storage_bytes();
    let declared = size_of::<ChildQualification>() + 4096 + 256;
    assert!(internal + declared <= original);
    assert_eq!(original, 16_384);
    println!(
        "preview_original_control_internal={internal} qualifier={} max_path=4096 max_download_name=256 total={} original={original}",
        size_of::<ChildQualification>(),
        internal + declared
    );

    assert!(size_of::<ChildQualification>().checked_add(usize::MAX).is_none());
    println!("child_qualification_storage_bytes={}", size_of::<ChildQualification>());
}
