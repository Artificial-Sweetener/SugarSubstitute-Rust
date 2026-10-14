// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise the actual section installer through each sealed native lease purpose.
use super::super::native;
use crate::{
    native::Signal,
    runtime_test_support::{Rig, TestResult, admitted_context},
};
use sugarsubstitute_contracts::{
    backend_custody::{BackendCustodyKey, BackendCustodySession},
    incident::DiagnosticRole,
    lifecycle::{CoordinatorEpoch, CoordinatorKey, NamespaceId},
};

fn key() -> TestResult<BackendCustodyKey> {
    Ok(BackendCustodyKey::new(
        CoordinatorKey {
            namespace: NamespaceId::new([1; 32]),
            epoch: CoordinatorEpoch::new([2; 16])?,
        },
        BackendCustodySession::new(9)?,
    ))
}

#[test]
fn backend_section_install_accepts_its_real_small_mapping_and_returns_original_claim() -> TestResult
{
    let rig = Rig::new()?;
    let (_scope, context) = admitted_context(&rig, 2, 1)?;
    let original = crate::windows_lifecycle_pipe::LifecyclePipeDeadline::new(1, 1)?;
    for mut lease in [
        native::Lease::backend_custody_server(key()?),
        native::Lease::backend_custody_client(original),
    ] {
        lease.install_captured_mapping(
            context.memory().claim(4096)?,
            Signal::new(&rig.client.core),
            4096,
        )?;
        assert!(context.memory().claim(1).is_err());
        drop(lease);
        drop(context.memory().claim(4096)?);
    }
    Ok(())
}

#[test]
fn desktop_and_diagnostic_sections_reject_backend_extent_before_taking_storage() -> TestResult {
    let rig = Rig::new()?;
    let (_scope, context) = admitted_context(&rig, 2, 1)?;
    let original = crate::windows_lifecycle_pipe::LifecyclePipeDeadline::new(1, 1)?;
    let mut desktop = native::Lease::client(original);
    let failure = desktop
        .install_captured_mapping(
            context.memory().claim(4096)?,
            Signal::new(&rig.client.core),
            4096,
        )
        .err()
        .ok_or("Desktop accepted undersized backend mapping")?;
    assert_eq!(failure.native_cause().stage(), "captured_mapping_extent");
    drop(context.memory().claim(4096)?);
    let mut diagnostic = native::Lease::diagnostic_client(DiagnosticRole::Storage, original, None);
    let failure = diagnostic
        .install_captured_mapping(
            context.memory().claim(4096)?,
            Signal::new(&rig.client.core),
            super::codec::MAX_LOGICAL_BYTES,
        )
        .err()
        .ok_or("Diagnostic accepted captured mapping")?;
    assert_eq!(failure.native_cause().stage(), "captured_mapping_purpose");
    drop(context.memory().claim(4096)?);
    Ok(())
}
