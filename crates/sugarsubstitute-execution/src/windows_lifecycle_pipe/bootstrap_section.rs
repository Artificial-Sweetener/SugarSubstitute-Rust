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
}
impl SectionOwner {
    pub(crate) fn new() -> Self {
        Self { master: Handle::new(), inherited: Handle::new(), view: 0, cause: None }
    }
    pub(crate) fn publish(
        &mut self,
        descriptor: &mut security::Descriptor,
        bytes: &[u8; BOOTSTRAP_BYTES],
    ) -> Result<(), LifecyclePipeFailure> {
        let attributes = descriptor.attributes();
        self.master.reserve("pipe_create_bootstrap")?;
        // SAFETY: unnamed page-file-backed fixed1KiB section, non-inheritable creator handle.
        let raw = unsafe {
            CreateFileMappingW(
                INVALID_HANDLE_VALUE,
                Some(&attributes),
                PAGE_READWRITE,
                0,
                BOOTSTRAP_BYTES as u32,
                None,
            )
        }
        .map_err(|e| error("pipe_create_bootstrap", e))?;
        adopt(&mut self.master, raw, "pipe_create_bootstrap")?;
        self.map(FILE_MAP_WRITE, "pipe_map_bootstrap")?;
        // SAFETY: fixed mapping has exactly1KiB writable committed storage; no child handle exists yet.
        unsafe {
            std::ptr::copy_nonoverlapping(bytes.as_ptr(), self.view as *mut u8, BOOTSTRAP_BYTES)
        };
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
        self.master.check(stage)?;
        // SAFETY: exact retained section; adopt the fixed view before fresh validation.
        let mapped = unsafe { MapViewOfFile(self.master.raw(), access, 0, 0, BOOTSTRAP_BYTES) };
        self.view = mapped.Value as usize;
        if self.view == 0 {
            return Err(error(stage, last_error()));
        }
        self.master.check(stage)?;
        Ok(())
    }
    fn unmap(&mut self) -> Result<(), LifecyclePipeFailure> {
        if self.view != 0 {
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
        }
        Ok(())
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
    pub(crate) fn attach(
        &mut self,
        handles: Arc<sugarsubstitute_contracts::execution::NativeHandleClaim>,
        original: crate::native_resources::EffectContext,
    ) {
        if let Some(owner) = &mut self.owner {
            owner.master.attach(handles.clone(), original.clone());
            owner.inherited.attach(handles, original);
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
