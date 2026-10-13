// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Construct restricted pipe/bootstrap ownership only after admitted storage exists.
use super::*;
impl Lease {
    pub(super) fn prepare_server(
        &mut self,
        context: &JobContext,
        handshake: Moment,
    ) -> Result<usize, LifecyclePipeFailure> {
        if let Some(parent) = &self.captured_parent {
            parent.check(context)?;
        }
        // SAFETY: The current-process pseudo handle remains valid and is never closed.
        let identity =
            security::process_identity(unsafe { GetCurrentProcess() }, &mut self.own_token)?;
        self.identity = Some(identity);
        let counter = crate::counter_deadline::encode_sampled(
            crate::windows_counter::sample,
            || Ok(context.now()),
            handshake,
        )
        .map_err(LifecyclePipeFailure::native)?;
        let deadline = LifecyclePipeDeadline::new(counter.ticks(), counter.frequency())
            .map_err(|cause| error("pipe_handshake_deadline", cause))?;
        let binding = self.binding.ok_or_else(|| failure("pipe_server_binding"))?;
        let name = bootstrap::name(binding);
        let mut descriptor =
            security::Descriptor::new(identity, security::DescriptorPurpose::RestrictedEndpoint)?;
        let attributes = descriptor.attributes();
        self.pipe.reserve("pipe_create_local_server")?;
        // SAFETY: bounded NUL-terminated local name, explicit ACL, sole instance and overlapped mode.
        let raw = unsafe {
            CreateNamedPipeW(
                PCWSTR(name.as_ptr()),
                PIPE_ACCESS_DUPLEX | FILE_FLAG_OVERLAPPED | FILE_FLAG_FIRST_PIPE_INSTANCE,
                PIPE_TYPE_MESSAGE | PIPE_READMODE_MESSAGE | PIPE_WAIT | PIPE_REJECT_REMOTE_CLIENTS,
                1,
                MAX_LIFECYCLE_MESSAGE_BYTES as u32,
                MAX_LIFECYCLE_MESSAGE_BYTES as u32,
                0,
                Some(&attributes),
            )
        };
        adopt(&mut self.pipe, raw, "pipe_create_local_server")?;
        let mut capability = [0; 32];
        // SAFETY: OS CSPRNG writes exactly this fixed admitted secret, without external buffers.
        let random = unsafe {
            windows::Win32::Security::Cryptography::BCryptGenRandom(
                None,
                &mut capability,
                windows::Win32::Security::Cryptography::BCRYPT_USE_SYSTEM_PREFERRED_RNG,
            )
        };
        random.ok().map_err(|e| error("pipe_capability_random", e))?;
        // SAFETY: The current-process pseudo handle is valid for identity queries.
        let (pid, birth) =
            security::process_facts(unsafe { GetCurrentProcess() }, &self.own_token)?;
        let data = bootstrap::Data {
            binding,
            pid,
            birth,
            session: identity.session,
            deadline,
            capability,
            name,
        };
        self.data = Some(data);
        let data = self.data.as_ref().ok_or_else(|| failure("pipe_bootstrap_owner"))?;
        if let Some(parent) = &self.captured_parent {
            parent.check(context)?;
            let source = crate::counter_deadline::encode_sampled(
                crate::windows_counter::sample,
                || Ok(context.now()),
                parent.pin.shared.expected.attach_budget().until(),
            )
            .map_err(LifecyclePipeFailure::native)?;
            let body =
                parent.startup.body().map_err(|cause| error("captured_startup_body", cause))?;
            self.section.publish_captured(
                &mut descriptor,
                data,
                parent.endpoint,
                parent.processes,
                source,
                body,
            )?;
        } else {
            let bytes = bootstrap::encode(data)?;
            self.section.publish(&mut descriptor, &bytes)?;
        }
        Ok(self.section.inherited())
    }
    pub(super) fn prepare_client(
        &mut self,
        context: &JobContext,
    ) -> Result<usize, LifecyclePipeFailure> {
        let original = self.original.ok_or_else(|| failure("pipe_client_deadline"))?;
        if self.captured_client {
            self.prepare_captured_child(context)?;
        } else {
            self.section.accept_input(&mut self.input);
            let bytes = self.section.read()?;
            self.data = Some(bootstrap::decode(&bytes, original, self.expected)?);
        }
        // SAFETY: The current-process pseudo handle remains valid and is never closed.
        let identity =
            security::process_identity(unsafe { GetCurrentProcess() }, &mut self.own_token)?;
        self.identity = Some(identity);
        let data = self.data.as_ref().ok_or_else(|| failure("pipe_bootstrap_owner"))?;
        if data.session != identity.session {
            return Err(failure("pipe_bootstrap_session"));
        }
        self.pipe.check("pipe_client_counter")?;
        let (now, frequency) =
            crate::windows_counter::sample().map_err(LifecyclePipeFailure::native)?;
        self.pipe.check("pipe_client_counter")?;
        if frequency != original.frequency as u64 || now >= original.ticks as u64 {
            return Err(failure("pipe_expired_bootstrap"));
        }
        self.binding = Some(data.binding);
        Ok(0)
    }
    /// Claim only pre-reserved custody while the actual connect admission is fenced.
    pub(super) fn claim_client_open(
        &mut self,
    ) -> Result<ClientOpenStage<'_>, LifecyclePipeFailure> {
        let data = self.data.as_ref().ok_or_else(|| failure("pipe_bootstrap_owner"))?;
        self.pipe.reserve_custody("pipe_open_local_client")?;
        Ok(ClientOpenStage { pipe: &mut self.pipe, name: &data.name })
    }
    pub(super) fn configure_client(&mut self) -> Result<(), LifecyclePipeFailure> {
        self.pipe.check("pipe_open_local_client")?;
        self.pipe.check("pipe_client_message_mode")?;
        // SAFETY: exact owned pipe; client handles initially default to byte-read mode.
        unsafe {
            SetNamedPipeHandleState(self.pipe.raw(), Some(&PIPE_READMODE_MESSAGE), None, None)
        }
        .map_err(|e| error("pipe_client_message_mode", e))?;
        self.pipe.check("pipe_client_message_mode")?;
        Ok(())
    }
}

/// Own the exact accepted finite-open stage; no native call occurs while claiming it.
pub(super) struct ClientOpenStage<'a> {
    pipe: &'a mut Handle,
    name: &'a [u16],
}
impl ClientOpenStage<'_> {
    /// Execute accepted finite work on the same native worker after all owner fences leave.
    pub(super) fn open(self) -> Result<(), LifecyclePipeFailure> {
        self.run(|name| {
            // SAFETY: exact validated local name, restricted access/SQOS and overlapped mode.
            unsafe {
                CreateFileW(
                    PCWSTR(name.as_ptr()),
                    CLIENT_ACCESS,
                    FILE_SHARE_MODE(0),
                    None,
                    OPEN_EXISTING,
                    FILE_FLAG_OVERLAPPED | SECURITY_SQOS_PRESENT | SECURITY_IDENTIFICATION,
                    None,
                )
            }
        })
    }
    fn run(
        self,
        native: impl FnOnce(&[u16]) -> windows::core::Result<HANDLE>,
    ) -> Result<(), LifecyclePipeFailure> {
        let result = native(self.name);
        match result {
            Ok(raw) => adopt(self.pipe, raw, "pipe_open_local_client").map_err(Into::into),
            Err(cause) => {
                self.pipe.adopt_partial(HANDLE::default());
                let mut failure = error("pipe_open_local_client", cause);
                if let Err(after) = self.pipe.check("pipe_open_local_client") {
                    failure.cause.retain_cleanup(after.cause);
                }
                Err(failure)
            }
        }
    }
}

#[cfg(test)]
#[path = "finite_stage_tests.rs"]
mod tests;
