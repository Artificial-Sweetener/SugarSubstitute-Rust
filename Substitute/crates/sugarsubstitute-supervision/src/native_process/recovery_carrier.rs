// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Carry only the original prepared Job and genuine Root sections into the fresh supervisor.
use super::*;
use std::os::windows::io::IntoRawHandle;
use windows::Win32::Foundation::{HANDLE_FLAG_INHERIT, SetHandleInformation};
impl Family {
    /// Set inheritance only on the actual Root-created empty recovery Job under its finite owner.
    pub(crate) fn secure_recovery_inheritance(
        &mut self,
        sid: &[u8],
        budget: &crate::native_claims::files::Budget,
        grant: &sugarsubstitute_execution::RegisteredRecoveryGrant,
        context: &sugarsubstitute_contracts::execution::JobContext,
    ) -> Result<(), RegistryError> {
        let mut phase =
            crate::durable_storage::protected::native_phase::NativePhase::ordinary(budget)
                .map_err(|cause| RegistryError::new("recovery_job_security_phase", cause))?;
        let descriptor =
            crate::durable_storage::protected::security::Descriptor::recovery_job(sid, &mut phase)
                .map_err(|cause| RegistryError::new("recovery_job_security_descriptor", cause))?;
        let attributes = descriptor.attributes();
        grant
            .check_context(context)
            .map_err(|cause| RegistryError::new("recovery_job_original", cause))?;
        self.check_native()?;
        // SAFETY: the retained descriptor and exact owned Job remain live through this call.
        unsafe {
            windows::Win32::Security::SetKernelObjectSecurity(
                handle(self.job, "recovery_job_security")?,
                windows::Win32::Security::OWNER_SECURITY_INFORMATION
                    | windows::Win32::Security::DACL_SECURITY_INFORMATION
                    | windows::Win32::Security::PROTECTED_DACL_SECURITY_INFORMATION
                    | windows::Win32::Security::LABEL_SECURITY_INFORMATION,
                windows::Win32::Security::PSECURITY_DESCRIPTOR(attributes.lpSecurityDescriptor),
            )
        }
        .map_err(|cause| RegistryError::new("recovery_job_security", cause))?;
        grant
            .check_context(context)
            .map_err(|cause| RegistryError::new("recovery_job_original", cause))?;
        self.check_native()?;
        // SAFETY: this exact original Job is uniquely retained; no ambient allowlist is used.
        unsafe {
            SetHandleInformation(
                handle(self.job, "recovery_inherited_job")?,
                HANDLE_FLAG_INHERIT.0,
                HANDLE_FLAG_INHERIT,
            )
        }
        .map_err(|cause| RegistryError::new("recovery_job_inheritable", cause))?;
        grant
            .check_context(context)
            .map_err(|cause| RegistryError::new("recovery_job_security_readback", cause))?;
        crate::durable_storage::protected::security::verify_recovery_job_borrowed(
            handle(self.job, "recovery_job_security_readback")?,
            budget,
            sid,
            &mut phase,
        )
        .map_err(|cause| RegistryError::new("recovery_job_security_readback", cause))
    }
    /// Move genuine paired sections/enrollment before any native attribute borrows their handles.
    pub(crate) fn adopt_recovery_carrier(
        &mut self,
        input: &mut WindowsLaunchInput,
    ) -> Result<(), RegistryError> {
        if let Some(carrier) = &input.recovery_carrier {
            carrier
                .grant
                .check()
                .map_err(|cause| RegistryError::new("recovery_carrier_source", cause))?;
            if self.attributes_initialized
                || self.recovery_carrier.is_some()
                || input.lifecycle.is_some()
                || input.fixture_events.is_some()
                || input.purpose.is_some()
                || input.backend_custody.is_some()
                || self.job.is_none()
            {
                return Err(RegistryError::invalid("recovery_carrier_input"));
            }
        }
        self.recovery_carrier = input.recovery_carrier.take();
        Ok(())
    }
    /// Build the fixed six-slot allowlist from three exact retained native owners and stdio.
    pub(crate) fn prepare_recovery_carrier_inheritance(&mut self) -> Result<(), RegistryError> {
        let carrier = self
            .recovery_carrier
            .as_ref()
            .ok_or_else(|| RegistryError::invalid("recovery_carrier_owner"))?;
        carrier
            .grant
            .check()
            .map_err(|cause| RegistryError::new("recovery_carrier_source", cause))?;
        let [recovery, desktop] = carrier.bootstrap.borrow_inheritance();
        if recovery.capture_processes.is_some() || desktop.capture_processes.is_some() {
            return Err(RegistryError::invalid("recovery_carrier_capture_conflict"));
        }
        let stdio = if let Some(pin) = &self.output_pin {
            let identity = self
                .output_identity
                .ok_or_else(|| RegistryError::invalid("recovery_carrier_output"))?;
            Some(
                pin.handles(identity)
                    .map_err(|cause| RegistryError::new("recovery_carrier_stdio", cause))?,
            )
        } else {
            None
        };
        let sources = [
            recovery.section.as_raw_handle() as usize,
            desktop.section.as_raw_handle() as usize,
            self.job.ok_or_else(|| RegistryError::invalid("recovery_carrier_job"))?,
        ];
        let (handles, count) = carrier_allowlist(sources, stdio)?;
        self.inherited_handles = handles;
        self.inherited_count = count;
        self.stdio_handles = stdio.unwrap_or([0; 3]);
        Ok(())
    }
}
impl WindowsPort {
    /// Authenticate the actual suspended carrier before either helper or Root permits resume.
    pub(super) fn enroll_recovery_carrier(
        &self,
        family: &mut Family,
        run: RunKey,
        key: ProcessKey,
    ) -> Result<(), RegistryError> {
        let trusted = family
            .recovery_carrier
            .as_ref()
            .ok_or_else(|| RegistryError::invalid("recovery_carrier_owner"))?
            .grant
            .key();
        if trusted.run() != run || key.owner != run.owner {
            return Err(RegistryError::invalid("recovery_carrier_binding"));
        }
        let input = self.duplicate_native(family, 0)?;
        #[cfg(not(test))]
        let WatchInput::Native(process) = input;
        #[cfg(test)]
        let WatchInput::Native(process) = input else {
            return Err(RegistryError::invalid("recovery_carrier_fake_input"));
        };
        let enrollment =
            family.recovery_carrier.as_mut().and_then(|carrier| carrier.enrollment.take());
        let Some(enrollment) = enrollment else {
            family.watch_duplicate = Some(process.into_raw_handle() as usize);
            return Err(RegistryError::invalid("recovery_carrier_enrollment"));
        };
        match enrollment.enroll(trusted, key, process) {
            Ok(()) => Ok(()),
            Err(rejected) => {
                family.watch_duplicate = Some(rejected.process.into_raw_handle() as usize);
                if let Some(carrier) = &mut family.recovery_carrier {
                    carrier.enrollment = Some(rejected.enrollment);
                }
                Err(RegistryError::new("recovery_carrier_enrollment", rejected.error))
            }
        }
    }
}
/// Admit only distinct original sections, Job and optional exact stdio within six slots.
fn carrier_allowlist(
    sources: [usize; 3],
    stdio: Option<[usize; 3]>,
) -> Result<([usize; 6], usize), RegistryError> {
    let mut values = [0; 6];
    values[..3].copy_from_slice(&sources);
    let count = if let Some(stdio) = stdio {
        values[3..].copy_from_slice(&stdio);
        6
    } else {
        3
    };
    for (index, value) in values[..count].iter().enumerate() {
        if *value == 0 || *value >= usize::MAX - 15 || values[..index].contains(value) {
            return Err(RegistryError::invalid("recovery_carrier_handle_alias"));
        }
    }
    Ok((values, count))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn carrier_allowlist_has_exact_two_sections_job_and_stdio() -> Result<(), RegistryError> {
        assert_eq!(carrier_allowlist([1, 2, 3], Some([4, 5, 6]))?, ([1, 2, 3, 4, 5, 6], 6));
        assert_eq!(carrier_allowlist([1, 2, 3], None)?, ([1, 2, 3, 0, 0, 0], 3));
        for sources in [[1, 1, 3], [1, 2, 2], [0, 2, 3], [1, 2, usize::MAX]] {
            assert!(carrier_allowlist(sources, None).is_err());
        }
        for stdio in [[1, 5, 6], [4, 4, 6], [4, 5, 0], [4, 5, usize::MAX]] {
            assert!(carrier_allowlist([1, 2, 3], Some(stdio)).is_err());
        }
        Ok(())
    }
}
