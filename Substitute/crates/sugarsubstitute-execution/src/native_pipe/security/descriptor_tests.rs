// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Observe the actual constructed ACE under admitted metadata custody, preserving restricted rights.
use super::*;
use crate::runtime_test_support::{Rig, TestResult, spec};
use sugarsubstitute_contracts::execution::{ExecutionPort, JobClass, JobOutcome};
fn mask(value: &Descriptor) -> Result<u32, NativePipeError> {
    let mut ace = std::ptr::null_mut();
    // SAFETY: the complete initialized ACL remains boxed and stable through this borrowed query.
    unsafe { GetAce(value.acl.as_ptr().cast(), 0, &mut ace) }?;
    // SAFETY: constructor produced the sole ACCESS_ALLOWED_ACE inside this retained ACL.
    let allowed = unsafe { &*ace.cast::<ACCESS_ALLOWED_ACE>() };
    assert_eq!(allowed.Header.AceType, 0);
    Ok(allowed.Mask)
}
#[test]
fn constructed_coordinator_ace_funds_duplex_instances_without_expanding_restricted_endpoint()
-> TestResult {
    let rig = Rig::new()?;
    let mut job = rig.client.submit_blocking(
        &rig.scope,
        spec(1, JobClass::InfrastructureOrdinary, 4096)?,
        |context| {
            let metadata = context.memory().claim(2048).map_err(|cause| {
                crate::native_pipe::handles::error("descriptor_metadata", cause)
            })?;
            let identity = Identity::fixture(1, 1, 1);
            let restricted = Descriptor::new(identity, DescriptorPurpose::RestrictedEndpoint)?;
            let server = Descriptor::new(identity, DescriptorPurpose::CoordinatorServer)?;
            let restricted_mask = mask(&restricted)?;
            let server_mask = mask(&server)?;
            assert_eq!(
                restricted_mask,
                FILE_READ_DATA.0
                    | FILE_WRITE_DATA.0
                    | FILE_READ_ATTRIBUTES.0
                    | FILE_WRITE_ATTRIBUTES.0
                    | SYNCHRONIZE.0
            );
            assert_eq!(restricted_mask & FILE_CREATE_PIPE_INSTANCE.0, 0);
            assert_eq!(
                server_mask
                    & (FILE_GENERIC_READ.0 | FILE_GENERIC_WRITE.0 | FILE_CREATE_PIPE_INSTANCE.0),
                FILE_GENERIC_READ.0 | FILE_GENERIC_WRITE.0 | FILE_CREATE_PIPE_INSTANCE.0
            );
            assert_eq!(
                server_mask
                    & (WRITE_DAC.0 | WRITE_OWNER.0 | DELETE.0 | TOKEN_ACCESS_SYSTEM_SECURITY.0),
                0
            );
            drop(server);
            drop(restricted);
            drop(metadata);
            Ok::<_, NativePipeError>(())
        },
    )?;
    assert!(matches!(rig.completion(&mut job)?.outcome, JobOutcome::Success(())));
    Ok(())
}
