// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Retain mapped-view and fixed output accounting through checked section retirement.
use super::*;
use windows::Win32::System::Memory::*;
struct SectionOwner {
    pub(crate) master: Handle,
    pub(crate) inherited: Handle,
    view: usize,
    cause: Option<NativeObservationError>,
    map_limit: usize,
    startup: Option<crate::windows_lifecycle_pipe::startup_mapping::StartupMapping>,
}
impl SectionOwner {
    pub(crate) fn new() -> Self {
        Self {
            master: Handle::new(),
            inherited: Handle::new(),
            view: 0,
            cause: None,
            map_limit: BOOTSTRAP_BYTES,
            startup: None,
        }
    }
    /// Preserve the supported fixed ordinary bootstrap through the same section owner.
    pub(crate) fn publish(
        &mut self,
        descriptor: &mut security::Descriptor,
        bytes: &[u8; BOOTSTRAP_BYTES],
    ) -> Result<(), LifecyclePipeFailure> {
        self.publish_with(descriptor, BOOTSTRAP_BYTES, |destination| {
            destination.copy_from_slice(bytes);
            Ok(())
        })
    }
    /// Write the original borrowed startup payload directly into the sole section.
    /// The entire mapping is adopted before validation or callback return.
    fn publish_with(
        &mut self,
        descriptor: &mut security::Descriptor,
        length: usize,
        encode: impl FnOnce(&mut [u8]) -> Result<(), LifecyclePipeFailure>,
    ) -> Result<(), LifecyclePipeFailure> {
        if length == 0 || length > crate::windows_lifecycle_pipe::captured::codec::MAX_LOGICAL_BYTES
        {
            return Err(failure("captured_section_extent"));
        }
        let attributes = descriptor.attributes();
        self.master.reserve("pipe_create_bootstrap")?;
        // SAFETY: unnamed page-file-backed fixed1KiB section, non-inheritable creator handle.
        let raw = unsafe {
            CreateFileMappingW(
                INVALID_HANDLE_VALUE,
                Some(&attributes),
                PAGE_READWRITE,
                0,
                length as u32,
                None,
            )
        }
        .map_err(|e| error("pipe_create_bootstrap", e))?;
        adopt(&mut self.master, raw, "pipe_create_bootstrap")?;
        self.map_extent(FILE_MAP_WRITE, length, "pipe_map_bootstrap")?;
        // SAFETY: this exact private writable mapping covers the requested serialized extent.
        // The child read-only duplicate is created only after this synchronous encoder returns.
        let destination = unsafe { std::slice::from_raw_parts_mut(self.view as *mut u8, length) };
        encode(destination)?;
        let mut read = HANDLE::default();
        self.inherited.reserve("pipe_duplicate_bootstrap_readonly")?;
        // SAFETY: duplicate narrows access to read-only; its output is retained before checking.
        let result = unsafe {
            DuplicateHandle(
                GetCurrentProcess(),
                self.master.raw(),
                GetCurrentProcess(),
                &mut read,
                FILE_MAP_READ.0,
                true,
                DUPLICATE_HANDLE_OPTIONS(0),
            )
        };
        self.inherited.finish_output(read, result, "pipe_duplicate_bootstrap_readonly")?;
        self.master.check("pipe_unmap_bootstrap")?;
        self.unmap()?;
        self.master.check("pipe_unmap_bootstrap")?;
        self.master.close_normal("pipe_close_bootstrap_writer")?;
        Ok(())
    }
    pub(crate) fn read(&mut self) -> Result<[u8; BOOTSTRAP_BYTES], LifecyclePipeFailure> {
        self.master.check("pipe_clear_bootstrap_inherit")?;
        // SAFETY: admitted child owns this inherited section handle; clearing does not close it.
        unsafe { SetHandleInformation(self.master.raw(), HANDLE_FLAG_INHERIT.0, HANDLE_FLAGS(0)) }
            .map_err(|e| error("pipe_clear_bootstrap_inherit", e))?;
        self.master.check("pipe_clear_bootstrap_inherit")?;
        self.map(FILE_MAP_READ, "pipe_map_inherited_bootstrap")?;
        let mut bytes = [0u8; BOOTSTRAP_BYTES];
        // SAFETY: successful fixed mapping exposes at least its requested1KiB range.
        unsafe {
            std::ptr::copy_nonoverlapping(
                self.view as *const u8,
                bytes.as_mut_ptr(),
                BOOTSTRAP_BYTES,
            )
        };
        self.master.check("pipe_unmap_bootstrap")?;
        self.unmap()?;
        self.master.check("pipe_unmap_bootstrap")?;
        self.master.close_normal("pipe_close_inherited_bootstrap")?;
        Ok(bytes)
    }
    fn map(&mut self, access: FILE_MAP, stage: &'static str) -> Result<(), LifecyclePipeFailure> {
        self.map_extent(access, BOOTSTRAP_BYTES, stage)
    }
    /// Adopt the exact returned mapping before any subsequent permission observation.
    fn map_extent(
        &mut self,
        access: FILE_MAP,
        length: usize,
        stage: &'static str,
    ) -> Result<(), LifecyclePipeFailure> {
        if length == 0 || length > self.map_limit {
            return Err(failure("captured_mapping_extent"));
        }
        if self.view != 0 {
            return Err(failure("pipe_live_map_reuse"));
        }
        self.master.check(stage)?;
        // SAFETY: exact retained section; adopt the fixed view before fresh validation.
        let mapped = unsafe { MapViewOfFile(self.master.raw(), access, 0, 0, length) };
        self.view = mapped.Value as usize;
        if self.view == 0 {
            return Err(error(stage, last_error()));
        }
        self.master.check(stage)?;
        Ok(())
    }
    fn unmap(&mut self) -> Result<(), LifecyclePipeFailure> {
        if self.view != 0 {
            if self.startup.as_ref().is_some_and(|owner| !owner.can_unmap()) {
                return Err(failure("captured_live_map_loan"));
            }
            // SAFETY: exact retained mapped-view base; failure retains it for explicit retry.
            if let Err(cause) = unsafe {
                UnmapViewOfFile(MEMORY_MAPPED_VIEW_ADDRESS { Value: self.view as *mut _ })
            } {
                let failure = error("pipe_unmap_bootstrap", cause);
                if let Some(first) = &mut self.cause {
                    first.retain_cleanup(failure.cause.clone());
                } else {
                    self.cause = Some(failure.cause.clone());
                }
                return Err(failure);
            }
            self.view = 0;
            if let Some(owner) = &mut self.startup {
                owner.unmapped();
            }
        }
        Ok(())
    }
    /// Read bounded prefix, checked-unmap it, then map the exact logical extent once.
    /// NoLoan controls physical borrowing only; the outside original claim remains held.
    fn read_captured(
        &mut self,
        original: LifecyclePipeDeadline,
    ) -> Result<
        (crate::windows_lifecycle_pipe::captured::codec::CapturedFacts, Data),
        LifecyclePipeFailure,
    > {
        self.master.check("captured_clear_section_inherit")?;
        // SAFETY: this actual admitted child exclusively owns the inherited section.
        unsafe { SetHandleInformation(self.master.raw(), HANDLE_FLAG_INHERIT.0, HANDLE_FLAGS(0)) }
            .map_err(|cause| error("captured_clear_section_inherit", cause))?;
        self.map_extent(FILE_MAP_READ, 2064, "captured_map_prefix")?;
        // SAFETY: the actual fixed prefix is mapped and retained until checked unmap.
        let prefix = unsafe { std::slice::from_raw_parts(self.view as *const u8, 2064) };
        let length = crate::windows_lifecycle_pipe::captured::codec::logical_bytes(prefix)?;
        self.master.check("captured_unmap_prefix")?;
        self.unmap()?;
        self.master.check("captured_remap_startup")?;
        self.map_extent(FILE_MAP_READ, length, "captured_map_startup")?;
        let mapping =
            self.startup.as_mut().ok_or_else(|| failure("captured_mapping_claim_missing"))?;
        // SAFETY: this retained exact immutable read-only view is owned by this Section.
        unsafe { mapping.adopt(self.view, length) }
            .map_err(|cause| error("captured_mapping_adopt", cause))?;
        // SAFETY: no loan exists yet; the same worker owns physical view and storage.
        let bytes = unsafe { std::slice::from_raw_parts(self.view as *const u8, length) };
        let (data, facts) =
            crate::windows_lifecycle_pipe::captured::codec::decode(bytes, original)?;
        Ok((facts, data))
    }
    pub(crate) fn retire(&mut self, pinned: bool) -> Result<bool, LifecyclePipeFailure> {
        self.unmap()?;
        self.master.close("pipe_close_bootstrap_master")?;
        if pinned {
            return Ok(false);
        }
        self.inherited.close("pipe_close_bootstrap_inheritance")?;
        Ok(true)
    }
}
pub(crate) struct Section {
    owner: Option<Box<SectionOwner>>,
}
impl Section {
    pub(crate) fn new() -> Self {
        Self { owner: Some(Box::new(SectionOwner::new())) }
    }
    /// Install the genuine original map claim before allocating mapped-control storage.
    pub(crate) fn install_mapping(
        &mut self,
        claim: sugarsubstitute_contracts::execution::MemoryClaim,
        signal: Arc<Signal>,
        extent: usize,
    ) -> Result<(), LifecyclePipeFailure> {
        let owner = self.owner.as_mut().ok_or_else(|| failure("pipe_mapping_owner"))?;
        if owner.startup.is_some() {
            return Err(failure("captured_mapping_duplicate"));
        }
        if extent < crate::windows_lifecycle_pipe::captured::codec::MAX_LOGICAL_BYTES {
            return Err(failure("captured_mapping_extent"));
        }
        owner.map_limit = extent;
        owner.startup = Some(crate::windows_lifecycle_pipe::startup_mapping::StartupMapping::new(
            claim, signal,
        ));
        Ok(())
    }
    /// Publish captured matching fields and original Entry bytes without allocating a copy.
    pub(crate) fn publish_captured(
        &mut self,
        descriptor: &mut security::Descriptor,
        data: &Data,
        endpoint: crate::capture_attach::types::CaptureEndpointObservation,
        processes: [usize; 2],
        counter: sugarsubstitute_contracts::execution::CounterDeadline,
        body: &[u8],
    ) -> Result<(), LifecyclePipeFailure> {
        let length = 2048 + 16 + body.len();
        self.owner.as_mut().ok_or_else(|| failure("pipe_mapping_owner"))?.publish_with(
            descriptor,
            length,
            |destination| {
                crate::windows_lifecycle_pipe::captured::codec::encode(
                    destination,
                    data,
                    endpoint,
                    processes,
                    counter,
                    body,
                )
            },
        )
    }
    /// Read one captured section; cache observations alone grant no authentication.
    pub(crate) fn read_captured(
        &mut self,
        original: LifecyclePipeDeadline,
    ) -> Result<
        (crate::windows_lifecycle_pipe::captured::codec::CapturedFacts, Data),
        LifecyclePipeFailure,
    > {
        self.owner.as_mut().ok_or_else(|| failure("pipe_mapping_owner"))?.read_captured(original)
    }
    /// Issue one of the actual two preclaimed native-owner mapping loans.
    pub(crate) fn loan(
        &mut self,
        range: std::ops::Range<usize>,
    ) -> Result<crate::windows_lifecycle_pipe::StartupViewLoan, LifecyclePipeFailure> {
        self.owner
            .as_mut()
            .and_then(|owner| owner.startup.as_mut())
            .ok_or_else(|| failure("captured_mapping_owner"))?
            .loan(range)
            .map_err(|cause| error("captured_mapping_loan", cause))
    }
    /// Count actual physical owner Box and both separately allocated Handle owner cells.
    pub(crate) const fn storage_bytes() -> usize {
        size_of::<SectionOwner>() + 2 * (Handle::storage_bytes() - size_of::<Handle>())
    }
    pub(crate) fn attach(&mut self, accounting: crate::native_pipe::accounting::OutputAccounting) {
        if let Some(owner) = &mut self.owner {
            owner.master.attach(accounting.clone());
            owner.inherited.attach(accounting);
        }
    }
    pub(crate) fn accept_input(&mut self, input: &mut Handle) {
        if let Some(owner) = &mut self.owner {
            input.transfer(&mut owner.master);
        }
    }
    pub(crate) fn inherited(&self) -> usize {
        self.owner.as_ref().map_or(0, |owner| owner.inherited.value())
    }
    pub(crate) fn publish(
        &mut self,
        descriptor: &mut security::Descriptor,
        bytes: &[u8; BOOTSTRAP_BYTES],
    ) -> Result<(), LifecyclePipeFailure> {
        self.owner.as_mut().ok_or_else(|| failure("pipe_mapping_owner"))?.publish(descriptor, bytes)
    }
    pub(crate) fn read(&mut self) -> Result<[u8; BOOTSTRAP_BYTES], LifecyclePipeFailure> {
        self.owner.as_mut().ok_or_else(|| failure("pipe_mapping_owner"))?.read()
    }
    pub(crate) fn retire(&mut self, pinned: bool) -> Result<bool, LifecyclePipeFailure> {
        self.owner.as_mut().ok_or_else(|| failure("pipe_mapping_owner"))?.retire(pinned)
    }
}
impl Drop for Section {
    fn drop(&mut self) {
        if let Some(owner) = self.owner.take()
            && (owner.view != 0 || owner.master.value() != 0 || owner.inherited.value() != 0)
        {
            let _retained = Box::leak(owner);
            tracing::error!(
                operation = "native_mapping_uncertain_drop",
                "Mapped view and its original handle/output custody retained before diagnostics"
            );
        }
    }
}

#[cfg(test)]
#[path = "bootstrap_section_tests.rs"]
mod tests;
