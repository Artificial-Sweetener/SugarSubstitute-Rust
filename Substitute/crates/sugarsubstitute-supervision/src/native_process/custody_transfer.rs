// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain genuine destination outputs until their exact authenticated adoption receipt.
use super::*;
use crate::{BackendCustodyAcknowledgement, BackendCustodyParent};
use windows::Win32::{
    Foundation::{DUPLICATE_SAME_ACCESS, DuplicateHandle, WAIT_TIMEOUT},
    System::Threading::{
        GetCurrentProcess, OpenProcess, PROCESS_DUP_HANDLE, PROCESS_QUERY_LIMITED_INFORMATION,
        PROCESS_SYNCHRONIZE, WaitForSingleObject,
    },
};

/// Actual native cells precede the same original parent metadata on every path.
pub(crate) struct CustodyDestination {
    data: Option<Box<Data>>,
    parent: sugarsubstitute_execution::backend_custody::BackendCustodyParentStorage,
}
struct Data {
    process: Option<usize>,
    identity: Option<Identity>,
    outputs: [Option<u64>; 2],
    nonce: Option<u64>,
}
impl CustodyDestination {
    /// Close local acknowledged cells after exact profile retirement under the original cutoff.
    pub(crate) fn close_profile_released(
        &mut self,
        ack: &BackendCustodyAcknowledgement,
        profile: sugarsubstitute_contracts::backend_custody::ProfileCustodyKey,
    ) -> Result<(), RegistryError> {
        if !ack.matches_storage(&self.parent) || ack.receipt()!=
            sugarsubstitute_contracts::backend_custody::BackendCustodyReceipt::ProfileReleased(profile) {
            return Err(RegistryError::invalid("profile_destination_release"));
        }
        let data = self
            .data
            .as_mut()
            .ok_or_else(|| RegistryError::invalid("custody_destination_closed"))?;
        data.outputs.fill(None);
        data.nonce = None;
        close_slot(&mut data.process, "profile_destination_close")
    }
    /// Allocate inert cells under the already-funded actual controller source.
    pub(crate) fn new(parent: &BackendCustodyParent) -> Self {
        Self {
            data: Some(Box::new(Data {
                process: None,
                identity: None,
                outputs: [None; 2],
                nonce: None,
            })),
            parent: parent.storage().clone(),
        }
    }
    /// Count the actual covered Box/data before its original controller preflight.
    pub(crate) const fn storage_bytes() -> usize {
        size_of::<Self>() + size_of::<Data>()
    }
    /// Open only the current original authenticated helper; retain outputs before queries.
    pub(crate) fn open(&mut self) -> Result<(), RegistryError> {
        let peer = self
            .parent
            .peer()
            .map_err(|cause| RegistryError::new("custody_destination_source", cause))?;
        let data = self
            .data
            .as_mut()
            .ok_or_else(|| RegistryError::invalid("custody_destination_closed"))?;
        if data.process.is_some() {
            return self.check();
        }
        // SAFETY: PID comes from the actual authenticated source, not caller facts.
        // The returned handle is immediately adopted, then its immutable birth is checked.
        let process = unsafe {
            OpenProcess(
                PROCESS_DUP_HANDLE | PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_SYNCHRONIZE,
                false,
                peer.process_id(),
            )
        }
        .map_err(|cause| RegistryError::new("custody_destination_open", cause))?;
        data.process = Some(process.0 as usize);
        data.identity =
            Some(Identity { pid: peer.process_id(), birth: peer.creation_time_100ns() });
        self.check()
    }
    /// Recheck original source, exact target birth and actual signal before each transfer.
    fn check(&self) -> Result<(), RegistryError> {
        let peer = self
            .parent
            .peer()
            .map_err(|cause| RegistryError::new("custody_destination_source", cause))?;
        let data = self
            .data
            .as_ref()
            .ok_or_else(|| RegistryError::invalid("custody_destination_closed"))?;
        let expected = Identity { pid: peer.process_id(), birth: peer.creation_time_100ns() };
        if data.identity != Some(expected) {
            return Err(RegistryError::invalid("custody_destination_identity"));
        }
        let process = handle(data.process, "custody_destination_process")?;
        let actual = membership::identity_checked(process, &mut || {
            self.parent
                .check()
                .map(|_| ())
                .map_err(|cause| RegistryError::new("custody_destination_source", cause))
        })?;
        if actual != expected {
            return Err(RegistryError::invalid("custody_destination_birth"));
        }
        // SAFETY: Borrow the exact durably held target; a zero-time signal query never waits.
        if unsafe { WaitForSingleObject(process, 0) } != WAIT_TIMEOUT {
            return Err(RegistryError::invalid("custody_destination_exited"));
        }
        Ok(())
    }
    /// Duplicate one already-owned source into that actual helper, retaining every output.
    pub(crate) fn duplicate(&mut self, source: HANDLE) -> Result<u64, RegistryError> {
        self.check()?;
        let data = self
            .data
            .as_mut()
            .ok_or_else(|| RegistryError::invalid("custody_destination_closed"))?;
        if data.nonce.is_some() {
            return Err(RegistryError::invalid("custody_destination_pending"));
        }
        let index = data
            .outputs
            .iter()
            .position(Option::is_none)
            .ok_or_else(|| RegistryError::invalid("custody_destination_capacity"))?;
        let mut output = HANDLE::default();
        // SAFETY: Caller owns the borrowed source throughout this operation. The
        // exact authenticated target remains owned. Its output is adopted before errors.
        let duplicated = unsafe {
            DuplicateHandle(
                GetCurrentProcess(),
                source,
                handle(data.process, "custody_destination_process")?,
                &mut output,
                0,
                false,
                DUPLICATE_SAME_ACCESS,
            )
        };
        if !output.is_invalid() {
            data.outputs[index] = Some(output.0 as usize as u64);
        }
        duplicated.map_err(|cause| RegistryError::new("custody_destination_duplicate", cause))?;
        data.outputs[index].ok_or_else(|| RegistryError::invalid("custody_destination_output"))
    }
    /// Bind issued outputs to their unchanged original transaction before ACK consumption.
    pub(crate) fn bind_nonce(&mut self, nonce: u64) -> Result<(), RegistryError> {
        let data = self
            .data
            .as_mut()
            .ok_or_else(|| RegistryError::invalid("custody_destination_closed"))?;
        if nonce == 0 || data.nonce.is_some() || data.outputs.iter().all(Option::is_none) {
            return Err(RegistryError::invalid("custody_destination_nonce"));
        }
        data.nonce = Some(nonce);
        Ok(())
    }
    /// Transfer output custody only through the actual same-controller sealed receipt.
    pub(crate) fn acknowledge(
        &mut self,
        ack: &BackendCustodyAcknowledgement,
    ) -> Result<(), RegistryError> {
        self.check()?;
        self.acknowledge_metadata(ack)
    }
    /// Retire only the metadata proven adopted by the sealed actual receipt.
    pub(crate) fn acknowledge_metadata(
        &mut self,
        ack: &BackendCustodyAcknowledgement,
    ) -> Result<(), RegistryError> {
        let data = self
            .data
            .as_mut()
            .ok_or_else(|| RegistryError::invalid("custody_destination_closed"))?;
        if !ack.matches_storage(&self.parent) || data.nonce != Some(ack.nonce()) {
            return Err(RegistryError::invalid("custody_destination_acknowledgement"));
        }
        data.outputs.fill(None);
        data.nonce = None;
        Ok(())
    }
    /// Duplicate this actual coordinator and return its kernel identity.
    pub(crate) fn duplicate_current_parent(&mut self) -> Result<(u64, Identity), RegistryError> {
        self.check()?;
        // SAFETY: Current-process pseudo handle is borrowed and never closed.
        let current = unsafe { GetCurrentProcess() };
        let identity = membership::identity(current)?;
        Ok((self.duplicate(current)?, identity))
    }
    /// Close local target only after actual helper exit makes unknown remote outputs absent.
    /// Failed queries/closes retain the exact cells and their original metadata.
    pub(crate) fn close_after_exit(&mut self) -> Result<bool, RegistryError> {
        let data = self
            .data
            .as_mut()
            .ok_or_else(|| RegistryError::invalid("custody_destination_closed"))?;
        if let Some(process) = data.process {
            // SAFETY: Exact original target remains owned; this query does not wait.
            let observed = unsafe { WaitForSingleObject(HANDLE(process as *mut _), 0) };
            if observed == windows::Win32::Foundation::WAIT_TIMEOUT {
                return Ok(false);
            }
            if observed != windows::Win32::Foundation::WAIT_OBJECT_0 {
                return Err(RegistryError::new(
                    "custody_destination_exit",
                    std::io::Error::last_os_error(),
                ));
            }
            data.outputs.fill(None);
            data.nonce = None;
            close_slot(&mut data.process, "custody_destination_close")?;
        }
        Ok(true)
    }
    /// Close original cells after this helper's actual same-generation release.
    /// The registry verifies physical local family closure before invoking this.
    pub(crate) fn close_released(
        &mut self,
        ack: &BackendCustodyAcknowledgement,
        resource: sugarsubstitute_contracts::lifecycle::ResourceKey,
    ) -> Result<(), RegistryError> {
        if !ack.matches_storage(&self.parent)
            || ack.receipt()
                != sugarsubstitute_contracts::backend_custody::BackendCustodyReceipt::Released(
                    resource,
                )
        {
            return Err(RegistryError::invalid("custody_destination_release"));
        }
        let data = self
            .data
            .as_mut()
            .ok_or_else(|| RegistryError::invalid("custody_destination_closed"))?;
        data.outputs.fill(None);
        data.nonce = None;
        close_slot(&mut data.process, "custody_destination_close")
    }
    /// Close an acknowledged local target under the creator's original close-only proof.
    /// Returns false while any actual unacknowledged destination output remains retained.
    pub(crate) fn close_acknowledged(
        &mut self,
        proof: &sugarsubstitute_execution::process_creation::ProcessCreationRetirement,
    ) -> Result<bool, RegistryError> {
        proof
            .check()
            .map_err(|cause| RegistryError::new("custody_destination_retirement", cause))?;
        let data = self
            .data
            .as_mut()
            .ok_or_else(|| RegistryError::invalid("custody_destination_closed"))?;
        if data.nonce.is_some() || data.outputs.iter().any(Option::is_some) {
            return Ok(false);
        }
        close_slot(&mut data.process, "custody_destination_close")?;
        Ok(true)
    }
}
impl Drop for CustodyDestination {
    fn drop(&mut self) {
        if let Some(data) = self.data.take() {
            if data.process.is_some() || data.outputs.iter().any(Option::is_some) {
                // No FFI, allocation or synthesized adoption occurs during exceptional drop.
                std::mem::forget((data, self.parent.clone()));
            } else {
                drop(data);
            }
        }
    }
}
