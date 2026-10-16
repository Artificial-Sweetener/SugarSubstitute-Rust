// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Validate actual inherited Root section custody before permitting a fresh capture-section writer.
use super::native::{bootstrap, peer};
use crate::{
    lifecycle_pipe::*, native_pipe::handles::Handle,
    recovery_supervisor::root_desktop::RootDesktopBootstrapLoan,
};
use sugarsubstitute_contracts::{
    execution::{CounterDeadline, JobContext, Moment},
    recovery_supervisor::RecoveryGrantKey,
};
use windows::Win32::Foundation::HANDLE;
/// Actual second inherited section, retained through checked read/unmap/close and authentication.
pub(in crate::windows_lifecycle_pipe) struct NativeRecoveryPair {
    input: Handle,
    section: bootstrap::Section,
    pending: Option<Pending>,
    ready: Option<RootDesktopBootstrapLoan>,
}
struct Pending {
    bytes: [u8; 1024],
    key: RecoveryGrantKey,
    counter: CounterDeadline,
    until: Moment,
    pid: u32,
    birth: u64,
    session: u32,
}
impl Drop for Pending {
    fn drop(&mut self) {
        self.bytes.fill(0);
    }
}
impl NativeRecoveryPair {
    /// Allocate inert owner cells only after the original fixed native metadata claim.
    pub(in crate::windows_lifecycle_pipe) fn new() -> Self {
        Self {
            input: Handle::new(),
            section: bootstrap::Section::new(),
            pending: None,
            ready: None,
        }
    }
    /// Account actual separate boxes and private Handle/section owner cells.
    pub(in crate::windows_lifecycle_pipe) const fn storage_bytes() -> usize {
        size_of::<Self>() + bootstrap::Section::storage_bytes() - size_of::<bootstrap::Section>()
            + Handle::storage_bytes()
            - size_of::<Handle>()
    }
    /// Reserve before the inherited-input transfer; rejection leaves both numeric slots unadopted.
    pub(in crate::windows_lifecycle_pipe) fn reserve_input(
        &mut self,
    ) -> Result<(), LifecyclePipeFailure> {
        self.input.reserve_custody("recovery_root_section_accounting").map_err(Into::into)
    }
    /// Adopt one exact exclusively inherited section into its pre-reserved physical owner.
    pub(in crate::windows_lifecycle_pipe) fn adopt(&mut self, raw: usize) {
        self.input.adopt_partial(HANDLE(raw as *mut _));
    }
    /// Read strict old Root Desktop schema; this alone cannot publish an authenticated loan.
    pub(in crate::windows_lifecycle_pipe) fn prepare(
        &mut self,
        recovery: &bootstrap::Data,
        context: &JobContext,
    ) -> Result<(), LifecyclePipeFailure> {
        if self.pending.is_some() || self.ready.is_some() {
            return Err(failure("recovery_root_section_replay"));
        }
        let key =
            recovery.binding.recovery().ok_or_else(|| failure("recovery_root_section_purpose"))?;
        self.section.accept_input(&mut self.input);
        let bytes = self.section.read()?;
        let (root, counter) = validate_pair_data(&bytes, recovery)?;
        let until = crate::counter_deadline::decode_sampled(
            || Ok(context.now()),
            crate::windows_counter::sample,
            counter,
        )
        .map_err(LifecyclePipeFailure::native)?;
        self.pending = Some(Pending {
            bytes,
            key,
            counter,
            until,
            pid: root.pid,
            birth: root.birth,
            session: root.session,
        });
        Ok(())
    }
    /// Publish only after capability and real same-Root kernel identity authentication succeeds.
    pub(in crate::windows_lifecycle_pipe) fn authenticate(
        &mut self,
        actual: VerifiedPeer,
    ) -> Result<(), LifecyclePipeFailure> {
        let pending =
            self.pending.as_mut().ok_or_else(|| failure("recovery_root_section_missing"))?;
        peer::birth((actual.pid, actual.birth), (pending.pid, pending.birth))?;
        if actual.session != pending.session || actual.binding.recovery() != Some(pending.key) {
            return Err(failure("recovery_root_section_authenticated_binding"));
        }
        let bytes = std::mem::replace(&mut pending.bytes, [0; 1024]);
        let loan = RootDesktopBootstrapLoan::from_native_authenticated(
            bytes,
            pending.key,
            pending.counter,
            pending.until,
            actual,
        )?;
        self.ready = Some(loan);
        drop(self.pending.take());
        Ok(())
    }
    /// Transfer the genuine same-process loan once; nothing serialized grants another loan.
    pub(in crate::windows_lifecycle_pipe) fn take(&mut self) -> Option<RootDesktopBootstrapLoan> {
        self.ready.take()
    }
    /// Retire every physical section/input output before actual transport terminal publication.
    pub(in crate::windows_lifecycle_pipe) fn retire(
        &mut self,
    ) -> Result<bool, LifecyclePipeFailure> {
        drop(self.ready.take());
        drop(self.pending.take());
        if !self.section.retire(false)? {
            return Ok(false);
        }
        self.input.close("recovery_close_root_section")?;
        Ok(true)
    }
}

/// Strict canonical original Root Desktop schema plus exact independently authenticated recovery Root.
fn validate_pair_data(
    bytes: &[u8; 1024],
    recovery: &bootstrap::Data,
) -> Result<(bootstrap::Data, CounterDeadline), LifecyclePipeFailure> {
    let key =
        recovery.binding.recovery().ok_or_else(|| failure("recovery_root_section_purpose"))?;
    let original = LifecyclePipeDeadline::new(
        i64::from_le_bytes(
            bytes[88..96].try_into().map_err(|_| failure("recovery_root_section_counter"))?,
        ),
        i64::from_le_bytes(
            bytes[96..104].try_into().map_err(|_| failure("recovery_root_section_frequency"))?,
        ),
    )
    .map_err(|cause| super::native::error("recovery_root_section_counter", cause))?;
    let root = bootstrap::decode(bytes, original, bootstrap::Kind::Desktop)?;
    if root.binding != PeerBinding::Desktop(key.run())
        || root.pid != recovery.pid
        || root.birth != recovery.birth
        || root.session != recovery.session
        || root.capability == [0; 32]
        || root.capability == recovery.capability
        || original.frequency() != recovery.deadline.frequency()
    {
        return Err(failure("recovery_root_section_binding"));
    }
    let counter =
        CounterDeadline::new(original.ticks().min(recovery.deadline.ticks()), original.frequency())
            .map_err(|cause| super::native::error("recovery_root_section_counter", cause))?;
    Ok((root, counter))
}
#[cfg(test)]
mod tests;
