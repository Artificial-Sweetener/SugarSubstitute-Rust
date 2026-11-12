// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Own one exact allowlist and all pins until actual attribute-list deletion.
use super::*;
use windows::Win32::System::Threading::{STARTF_USESTDHANDLES, STARTUPINFOW};
impl Family {
    pub(crate) fn has_native_ownership(&self) -> bool {
        self.job.is_some()
            || self.rejected_job.is_some()
            || self.thread.is_some()
            || self.watch_duplicate.is_some()
            || self.members.iter().flatten().any(|member| member.handle.is_some())
            || self.attributes_initialized
            || self.recovery_carrier.is_some()
            || self.fixture_events.is_some()
            || self.lifecycle_bootstrap.is_some()
            || self.diagnostic_bootstrap.is_some()
            || self.diagnostic_enrollment.is_some()
            || self.backend_custody.is_some()
            || self.output_pin.is_some()
    }
    pub(crate) fn adopt_fixture_events(
        &mut self,
        input: &mut WindowsLaunchInput,
    ) -> Result<(), RegistryError> {
        if self.attributes_initialized || self.fixture_events.is_some() {
            return Err(RegistryError::invalid("fixture_inheritance_owner"));
        }
        self.fixture_events = input.fixture_events.take();
        Ok(())
    }
    pub(crate) fn adopt_output(
        &mut self,
        input: &mut WindowsLaunchInput,
        binding: FamilyBinding,
    ) -> Result<(), RegistryError> {
        if self.attributes_initialized || self.output_pin.is_some() || self.output_cancel.is_some()
        {
            return Err(RegistryError::invalid("output_inheritance_owner"));
        }
        if let Some(output) = &input.output
            && (!output.validates(binding)
                || self.fixture_events.is_some()
                || self.diagnostic_bootstrap.is_some()
                || self.backend_custody.is_some())
        {
            return Err(RegistryError::invalid("output_inheritance_binding"));
        }
        if let Some(output) = input.output.take() {
            self.output_identity = Some(output.identity);
            // The unique pin is durable before any borrow or fallible native step.
            self.output_pin = Some(output.stdio);
        }
        Ok(())
    }
    pub(crate) fn prepare_inheritance(&mut self) -> Result<(), RegistryError> {
        if self.attributes_initialized {
            return Err(RegistryError::invalid("live_inheritance_reuse"));
        }
        if let Some(probe) = self.preparation_image.as_ref().and_then(|launch| launch.probe()) {
            if !probe.can_launch()
                || self.recovery_carrier.is_some()
                || self.lifecycle_bootstrap.is_some()
                || self.diagnostic_bootstrap.is_some()
                || self.backend_custody.is_some()
                || self.fixture_events.is_some()
                || self.output_pin.is_some()
            {
                return Err(RegistryError::invalid("updater_probe_inheritance_conflict"));
            }
            let (handles, count) = allowlist(
                None,
                Some(probe.exclusion_handle().as_raw_handle() as usize),
                None,
                None,
            )?;
            self.inherited_handles = handles;
            self.inherited_count = count;
            return Ok(());
        }
        if self.recovery_carrier.is_some() {
            return self.prepare_recovery_carrier_inheritance();
        }
        let fixture = if let Some(events) = &self.fixture_events {
            let view = events.borrow_inheritance();
            Some(
                [view.ready, view.release, view.challenge, view.response]
                    .map(|h| h.as_raw_handle() as usize),
            )
        } else {
            None
        };
        if self.backend_custody.is_some()
            && (self.lifecycle_bootstrap.is_some()
                || self.diagnostic_bootstrap.is_some()
                || self.fixture_events.is_some()
                || self.output_pin.is_some())
        {
            return Err(RegistryError::invalid("custody_inheritance_conflict"));
        }
        if self.diagnostic_bootstrap.is_some()
            && (self.lifecycle_bootstrap.is_some()
                || self.fixture_events.is_some()
                || self.output_pin.is_some())
        {
            return Err(RegistryError::invalid("diagnostic_inheritance_conflict"));
        }
        let bootstrap = self
            .lifecycle_bootstrap
            .as_ref()
            .map(LifecyclePipeBootstrap::borrow_inheritance)
            .or_else(|| {
                self.backend_custody.as_ref().map(|launch| launch.bootstrap.borrow_inheritance())
            });
        let section =
            bootstrap.as_ref().map(|view| view.section.as_raw_handle() as usize).or_else(|| {
                self.diagnostic_bootstrap.as_ref().map(|bootstrap| {
                    bootstrap.borrow_inheritance().section.as_raw_handle() as usize
                })
            });
        let captured = bootstrap
            .and_then(|view| view.capture_processes)
            .map(|handles| handles.map(|handle| handle.as_raw_handle() as usize));
        let stdio = if let Some(pin) = &self.output_pin {
            let identity =
                self.output_identity.ok_or_else(|| RegistryError::invalid("output_identity"))?;
            let stdio = pin
                .handles(identity)
                .map_err(|e| RegistryError::new("output_borrow_inheritance", e))?;
            Some(stdio)
        } else {
            None
        };
        let (handles, count) = allowlist(fixture, section, captured, stdio)?;
        self.stdio_handles = stdio.unwrap_or([0; 3]);
        self.inherited_handles = handles;
        self.inherited_count = count;
        Ok(())
    }
    pub(crate) fn configure_stdio(&self, startup: &mut STARTUPINFOW) {
        if self.output_pin.is_some() || self.recovery_stdio {
            startup.dwFlags |= STARTF_USESTDHANDLES;
            startup.hStdInput = HANDLE(self.stdio_handles[0] as *mut _);
            startup.hStdOutput = HANDLE(self.stdio_handles[1] as *mut _);
            startup.hStdError = HANDLE(self.stdio_handles[2] as *mut _);
        }
    }
    pub(crate) fn validate_output_for_resume(&self) -> Result<(), RegistryError> {
        if let Some(pin) = &self.output_pin {
            let identity =
                self.output_identity.ok_or_else(|| RegistryError::invalid("output_identity"))?;
            let handles = pin
                .handles(identity)
                .map_err(|cause| RegistryError::new("output_before_resume", cause))?;
            if handles != self.stdio_handles {
                return Err(RegistryError::invalid("output_before_resume_identity"));
            }
        }
        Ok(())
    }
    pub(crate) fn request_output_close(&self) {
        if let Some(pin) = &self.output_pin {
            pin.request_close();
        }
        if let Some(cancel) = &self.output_cancel {
            cancel.request_close();
        }
    }
    pub(crate) fn release_inheritance(&mut self) -> Result<(), RegistryError> {
        if self.attributes_initialized {
            return Err(RegistryError::invalid("output_release_before_attributes"));
        }
        drop(self.recovery_carrier.take());
        drop(self.fixture_events.take());
        drop(self.lifecycle_bootstrap.take());
        drop(self.diagnostic_bootstrap.take());
        drop(self.backend_custody.take());
        if let Some(pin) = self.output_pin.take() {
            self.output_cancel = Some(pin.release());
        }
        self.inherited_handles = [0; 6];
        self.inherited_count = 0;
        self.stdio_handles = [0; 3];
        self.recovery_stdio = false;
        Ok(())
    }
}
fn allowlist(
    fixture: Option<[usize; 4]>,
    section: Option<usize>,
    captured: Option<[usize; 2]>,
    stdio: Option<[usize; 3]>,
) -> Result<([usize; 6], usize), RegistryError> {
    if fixture.is_some() && (section.is_some() || captured.is_some() || stdio.is_some()) {
        return Err(RegistryError::invalid("fixture_inheritance_conflict"));
    }
    if captured.is_some() && section.is_none() {
        return Err(RegistryError::invalid("captured_inheritance_without_section"));
    }
    let mut handles = [0; 6];
    let mut count = 0;
    if let Some(fixture) = fixture {
        handles[..4].copy_from_slice(&fixture);
        count = 4;
    } else {
        if let Some(section) = section {
            handles[0] = section;
            count = 1;
        }
        if let Some(captured) = captured {
            handles[count..count + 2].copy_from_slice(&captured);
            count += 2;
        }
        if let Some(stdio) = stdio {
            handles[count..count + 3].copy_from_slice(&stdio);
            count += 3;
        }
    }
    for (i, handle) in handles[..count].iter().enumerate() {
        if *handle == 0 || *handle >= usize::MAX - 15 || handles[..i].contains(handle) {
            return Err(RegistryError::invalid("inherited_handle_identity"));
        }
    }
    Ok((handles, count))
}
#[cfg(test)]
#[path = "inheritance_tests.rs"]
mod tests;
