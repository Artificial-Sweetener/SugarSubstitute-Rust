// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Closed version-two origin envelope; ordinary version-one ABI remains in its owner.
use super::*;
/// Closed historical Coordinator or full Recovery correlation; current Root authority is separate.
#[repr(C)]
#[derive(Clone, Copy, Default, Eq, PartialEq)]
pub(in crate::native::finished_report) struct RecoveryOrigin {
    pub namespace: [u8; 32],
    pub root_epoch: [u8; 16],
    pub instance: u64,
    pub generation: u64,
    pub grant: u64,
    pub capture_epoch: [u8; 16],
    pub origin_kind: u32,
    pub reserved: u32,
}
impl RecoveryOrigin {
    pub(in crate::native::finished_report) fn from_origin(
        origin: sugarsubstitute_contracts::crash::CaptureGenerationOrigin,
        operation: sugarsubstitute_contracts::incident::LogicalStoreOperationKey,
    ) -> Result<Option<Self>, sugarsubstitute_contracts::lifecycle::IdentityError> {
        let sugarsubstitute_contracts::crash::CaptureGenerationOrigin::Recovery(recovery) = origin
        else {
            let owner = origin.coordinator();
            if operation.owner() == owner {
                return Ok(None);
            }
            return Ok(Some(Self {
                namespace: *owner.namespace.bytes(),
                root_epoch: *owner.epoch.bytes(),
                instance: 0,
                generation: 0,
                grant: 0,
                capture_epoch: *owner.epoch.bytes(),
                origin_kind: 1,
                reserved: 0,
            }));
        };
        let run = recovery.run();
        let capture = origin.capture_key()?;
        Ok(Some(Self {
            namespace: *run.owner.namespace.bytes(),
            root_epoch: *run.owner.epoch.bytes(),
            instance: run.instance.get(),
            generation: run.generation.get(),
            grant: recovery.grant().get(),
            capture_epoch: *capture.epoch.bytes(),
            origin_kind: 2,
            reserved: 0,
        }))
    }
}
#[repr(C)]
pub(in crate::native::finished_report) struct Open2<'call> {
    pub version: u32,
    pub reserved: u32,
    pub origin: RecoveryOrigin,
    pub operation: Operation,
    pub query: Open<'call>,
}
#[repr(C)]
pub(in crate::native::finished_report) struct Discard2<'call> {
    pub version: u32,
    pub reserved: u32,
    pub origin: RecoveryOrigin,
    pub disposition: Discard<'call>,
}
#[repr(C)]
pub(in crate::native::finished_report) struct Recover2<'call> {
    pub version: u32,
    pub reserved: u32,
    pub origin: RecoveryOrigin,
    pub disposition: Recover<'call>,
}
