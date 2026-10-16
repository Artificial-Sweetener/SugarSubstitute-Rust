// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Write one combined section inside the actual finite suspended-child creation owner.
use super::*;
use crate::{
    native_pipe::{handles::Handle, security},
    process_creation::{ProcessCreationMemory, ProcessCreationRetirement},
};
use std::os::windows::io::BorrowedHandle;
use sugarsubstitute_contracts::execution::MemoryClaim;
use windows::Win32::System::Threading::GetCurrentProcess;
struct Owned {
    section: crate::windows_lifecycle_pipe::native::bootstrap::Section,
    token: Handle,
    parent: ParentSource,
    memory: ProcessCreationMemory,
    prepared: bool,
    attempted: bool,
    _claim: MemoryClaim,
}
/// The actual creation driver retains this nonclone owner through attribute deletion.
/// Checked retirement closes only local section/token outputs; fresh handler and UI
/// process custody remain with their separate registration/family owners.
pub struct RecoveryDesktopBootstrap {
    owner: Option<Box<Owned>>,
}
impl RecoveryDesktopBootstrap {
    /// Determine the exact section/page and fixed native-owner reservation before admission.
    /// This is a storage requirement within existing quotas, not another source capability.
    /// # Errors
    /// Rejects unsupported platform page facts or an owner exceeding its fixed4KiB control.
    #[allow(unsafe_code)]
    pub fn required_reservation_bytes() -> Result<u64, lifecycle_pipe::LifecyclePipeFailure> {
        let mut facts = windows::Win32::System::SystemInformation::SYSTEM_INFO::default();
        // SAFETY: the OS fills only this initialized fixed page-fact output.
        unsafe { windows::Win32::System::SystemInformation::GetSystemInfo(&mut facts) };
        let page = facts.dwPageSize as usize;
        let control = size_of::<Owned>()
            + crate::windows_lifecycle_pipe::native::bootstrap::Section::storage_bytes()
            + Handle::storage_bytes()
            + 1024;
        if !page.is_power_of_two() || control > 4096 {
            return Err(lifecycle_pipe::failure("recovery_section_layout"));
        }
        let extent = super::super::codec::MAX_LOGICAL_BYTES
            .checked_add(page - 1)
            .map(|n| n & !(page - 1))
            .and_then(|n| n.checked_add(4096))
            .ok_or_else(|| lifecycle_pipe::failure("recovery_section_layout"))?;
        Ok(extent as u64)
    }
    /// Transfer the two genuine capture/startup owners after their final actual Core fence.
    /// The caller has admitted the exact additional reservation in this same creation job.
    /// # Errors
    /// Every rejection before transfer preserves `input`; no native effects occur here.
    pub fn new(
        input: &mut CapturedLifecycleStart,
        memory: &ProcessCreationMemory,
    ) -> Result<Self, lifecycle_pipe::LifecyclePipeFailure> {
        memory.check_effect().map_err(|cause| fail("recovery_section_original", cause))?;
        let (pin, startup) = input
            .owners
            .as_ref()
            .ok_or_else(|| lifecycle_pipe::failure("recovery_section_input"))?;
        let source = startup
            .recovery_source()
            .ok_or_else(|| lifecycle_pipe::failure("recovery_section_source"))?;
        let run = source.key.run();
        let binding = AdmissionBinding::project(
            run,
            source.until,
            pin,
            startup.admission_binding().map_err(|cause| fail("recovery_section_startup", cause))?,
        );
        let context =
            memory.original_context().map_err(|cause| fail("recovery_section_context", cause))?;
        let claim = context
            .memory()
            .claim(Self::required_reservation_bytes()?)
            .map_err(|cause| fail("recovery_section_memory", cause))?;
        let (endpoint, processes) = {
            let state = lock(&pin.shared.state);
            (
                state
                    .endpoint
                    .ok_or_else(|| lifecycle_pipe::failure("recovery_section_endpoint"))?,
                state
                    .handles
                    .ok_or_else(|| lifecycle_pipe::failure("recovery_section_processes"))?,
            )
        };
        let core = pin
            .shared
            .custody
            .core
            .upgrade()
            .ok_or_else(|| lifecycle_pipe::failure("recovery_section_core"))?;
        let now = context.now();
        {
            let state = lock(&core.state);
            binding
                .validate_current_locked(&core, &state, now)
                .map_err(|cause| fail("recovery_section_current", cause))?;
        }
        memory
            .claim_recovery_section(source)
            .map_err(|cause| fail("recovery_section_once", cause))?;
        let Some((pin, startup)) = input.owners.take() else {
            return Err(lifecycle_pipe::failure("recovery_section_input"));
        };
        let mut section = crate::windows_lifecycle_pipe::native::bootstrap::Section::new();
        let mut token = Handle::new();
        let accounting = crate::native_pipe::accounting::OutputAccounting::Creation(memory.clone());
        section.attach(accounting.clone());
        token.attach(accounting);
        Ok(Self {
            owner: Some(Box::new(Owned {
                section,
                token,
                parent: ParentSource { pin, startup: Some(startup), binding, endpoint, processes },
                memory: memory.clone(),
                prepared: false,
                attempted: false,
                _claim: claim,
            })),
        })
    }
    /// Publish from original Root bytes after checking the real current creator identity.
    /// A partial output stays in this owner and a second call cannot create another section.
    /// # Errors
    /// Preserves genuine source, kernel, original deadline and checked-map failures.
    #[allow(unsafe_code)]
    pub fn prepare(&mut self) -> Result<(), lifecycle_pipe::LifecyclePipeFailure> {
        let owner =
            self.owner.as_mut().ok_or_else(|| lifecycle_pipe::failure("recovery_section_owner"))?;
        if owner.attempted {
            return Err(lifecycle_pipe::failure("recovery_section_replay"));
        }
        owner.memory.check_effect().map_err(|cause| fail("recovery_section_original", cause))?;
        let context = owner
            .memory
            .original_context()
            .map_err(|cause| fail("recovery_section_context", cause))?;
        owner.parent.check(context)?;
        owner.attempted = true;
        // SAFETY: current-process pseudo handle is observed, never adopted or closed.
        let process = unsafe { GetCurrentProcess() };
        let identity = security::process_identity(process, &mut owner.token)?;
        let actual = security::process_facts(process, &owner.token)?;
        let expected = owner.parent.endpoint.expectation().creator();
        if actual != (expected.process_id(), expected.creation_time_100ns()) {
            return Err(lifecycle_pipe::failure("recovery_section_actual_creator"));
        }
        let startup = owner
            .parent
            .startup
            .as_ref()
            .ok_or_else(|| lifecycle_pipe::failure("recovery_section_startup"))?;
        let origin = startup
            .recovery_source()
            .ok_or_else(|| lifecycle_pipe::failure("recovery_section_source"))?;
        let root = origin.root_desktop.bytes();
        if u32::from_le_bytes(
            root[76..80]
                .try_into()
                .map_err(|_| lifecycle_pipe::failure("recovery_section_session"))?,
        ) != identity.session
        {
            return Err(lifecycle_pipe::failure("recovery_section_session"));
        }
        let cutoff = origin.until.min(owner.parent.pin.shared.expected.attach_budget().until());
        let sampled = crate::counter_deadline::encode_sampled(
            crate::windows_counter::sample,
            || Ok(context.now()),
            cutoff,
        )
        .map_err(lifecycle_pipe::LifecyclePipeFailure::native)?;
        if sampled.frequency() != origin.root_desktop.counter().frequency() {
            return Err(lifecycle_pipe::failure("recovery_section_frequency"));
        }
        let source = sugarsubstitute_contracts::execution::CounterDeadline::new(
            sampled.ticks().min(origin.root_desktop.counter().ticks()),
            sampled.frequency(),
        )
        .map_err(|cause| fail("recovery_section_counter", cause))?;
        let mut descriptor =
            security::Descriptor::new(identity, security::DescriptorPurpose::RestrictedEndpoint)?;
        owner.parent.check(context)?;
        owner.section.publish_recovery(
            &mut descriptor,
            root,
            origin.key,
            owner.parent.endpoint,
            owner.parent.processes,
            source,
            startup.body().map_err(|cause| fail("recovery_section_body", cause))?,
        )?;
        owner.memory.check_effect().map_err(|cause| fail("recovery_section_after", cause))?;
        owner.prepared = true;
        Ok(())
    }
    /// Return only the immutable Root-assigned Desktop facts from this actual prepared source.
    /// # Errors
    /// Rejects an unpublished or stopped original creation owner, incomplete source or counter.
    pub fn role_facts(
        &self,
    ) -> Result<
        (
            sugarsubstitute_contracts::startup::MainIdentity,
            crate::windows_lifecycle_pipe::LifecyclePipeDeadline,
        ),
        lifecycle_pipe::LifecyclePipeFailure,
    > {
        let owner = self
            .owner
            .as_ref()
            .filter(|owner| owner.prepared)
            .ok_or_else(|| lifecycle_pipe::failure("recovery_section_not_ready"))?;
        owner.memory.check_effect().map_err(|cause| fail("recovery_section_original", cause))?;
        let startup = owner
            .parent
            .startup
            .as_ref()
            .ok_or_else(|| lifecycle_pipe::failure("recovery_section_startup"))?;
        let origin = startup
            .recovery_source()
            .ok_or_else(|| lifecycle_pipe::failure("recovery_section_source"))?;
        let identity = origin
            .desktop_identity
            .ok_or_else(|| lifecycle_pipe::failure("recovery_section_identity"))?;
        if identity.run != origin.key.run() {
            return Err(lifecycle_pipe::failure("recovery_section_identity"));
        }
        let original = origin.root_desktop.counter();
        let deadline = crate::windows_lifecycle_pipe::LifecyclePipeDeadline::new(
            original.ticks(),
            original.frequency(),
        )
        .map_err(|cause| fail("recovery_section_counter", cause))?;
        Ok((identity, deadline))
    }
    /// Borrow exactly one section and both original inherited process pins for HANDLE_LIST.
    /// # Errors
    /// Rejects an unpublished section or original cancellation/deadline; no effects occur.
    #[allow(unsafe_code)]
    pub fn borrow_inheritance(
        &self,
    ) -> Result<
        crate::windows_lifecycle_pipe::LifecyclePipeInheritance<'_>,
        lifecycle_pipe::LifecyclePipeFailure,
    > {
        let owner = self
            .owner
            .as_ref()
            .filter(|owner| owner.prepared)
            .ok_or_else(|| lifecycle_pipe::failure("recovery_section_not_ready"))?;
        owner.memory.check_effect().map_err(|cause| fail("recovery_section_original", cause))?;
        // SAFETY: this borrow retains the sole section owner and genuine capture pin.
        Ok(unsafe {
            crate::windows_lifecycle_pipe::LifecyclePipeInheritance {
                section: BorrowedHandle::borrow_raw(owner.section.inherited() as *mut _),
                capture_processes: Some(
                    owner.parent.processes.map(|raw| BorrowedHandle::borrow_raw(raw as *mut _)),
                ),
            }
        })
    }
    /// Check-close the exact native outputs under the original creation cleanup proof.
    /// Failure preserves every live slot, first cause, source pin and original byte credit.
    /// # Errors
    /// Rejects foreign/expired retirement or an actual close/unmap failure.
    pub fn retire(
        &mut self,
        proof: &ProcessCreationRetirement,
    ) -> Result<bool, lifecycle_pipe::LifecyclePipeFailure> {
        let Some(owner) = self.owner.as_mut() else {
            return Ok(true);
        };
        if !proof.matches(&owner.memory) {
            return Err(lifecycle_pipe::failure("recovery_section_retirement"));
        }
        proof.check().map_err(|cause| fail("recovery_section_retirement", cause))?;
        owner.section.retire(false)?;
        proof.check().map_err(|cause| fail("recovery_section_retirement", cause))?;
        owner.token.close("recovery_section_token_close")?;
        let retired = owner.section.retired() && owner.token.value() == 0;
        if retired && let Some(owner) = self.owner.take() {
            release(owner);
        }
        Ok(retired)
    }
}
impl Drop for RecoveryDesktopBootstrap {
    fn drop(&mut self) {
        if let Some(owner) = self.owner.take() {
            if owner.section.retired() && owner.token.value() == 0 {
                release(owner);
            } else {
                let _retained = Box::leak(owner);
                tracing::error!(
                    operation = "recovery_section_uncertain_drop",
                    "Original section, native slots, capture pin and memory retained before diagnostics"
                );
            }
        }
    }
}
fn fail(
    stage: &'static str,
    cause: impl std::error::Error + Send + Sync + 'static,
) -> lifecycle_pipe::LifecyclePipeFailure {
    lifecycle_pipe::LifecyclePipeFailure::native(crate::native::NativeObservationError::new(
        stage, cause,
    ))
}

/// Moving fields out first frees the original Box before returning its outside capacity.
fn release(owner: Box<Owned>) {
    let Owned { section, token, parent, memory, prepared: _, attempted: _, _claim } = *owner;
    drop(section);
    drop(token);
    drop(parent);
    drop(memory);
    drop(_claim);
}
