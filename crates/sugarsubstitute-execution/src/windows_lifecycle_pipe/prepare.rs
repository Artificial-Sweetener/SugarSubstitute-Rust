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
        let run = self.run.ok_or_else(|| failure("pipe_server_run"))?;
        let name = bootstrap::name(run);
        let mut descriptor = security::Descriptor::new(identity)?;
        let attributes = descriptor.attributes();
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
        let (pid, birth) = security::process_facts(unsafe { GetCurrentProcess() })?;
        let data = bootstrap::Data {
            run,
            pid,
            birth,
            session: identity.session,
            deadline,
            capability,
            name,
        };
        let bytes = bootstrap::encode(&data)?;
        self.data = Some(data);
        self.section.publish(&mut descriptor, &bytes)?;
        Ok(self.section.inherited.0)
    }
    pub(super) fn prepare_client(&mut self) -> Result<usize, LifecyclePipeFailure> {
        let original = self.original.ok_or_else(|| failure("pipe_client_deadline"))?;
        self.section.master.0 = self.input.0;
        self.input.0 = 0;
        let bytes = self.section.read()?;
        self.data = Some(bootstrap::decode(&bytes, original)?);
        // SAFETY: The current-process pseudo handle remains valid and is never closed.
        let identity =
            security::process_identity(unsafe { GetCurrentProcess() }, &mut self.own_token)?;
        self.identity = Some(identity);
        let data = self.data.as_ref().ok_or_else(|| failure("pipe_bootstrap_owner"))?;
        if data.session != identity.session {
            return Err(failure("pipe_bootstrap_session"));
        }
        let (now, frequency) =
            crate::windows_counter::sample().map_err(LifecyclePipeFailure::native)?;
        if frequency != original.frequency as u64 || now >= original.ticks as u64 {
            return Err(failure("pipe_expired_bootstrap"));
        }
        // SAFETY: exact validated local name, explicit restricted access/SQOS and overlapped mode.
        let raw = unsafe {
            CreateFileW(
                PCWSTR(data.name.as_ptr()),
                CLIENT_ACCESS,
                FILE_SHARE_MODE(0),
                None,
                OPEN_EXISTING,
                FILE_FLAG_OVERLAPPED | SECURITY_SQOS_PRESENT | SECURITY_IDENTIFICATION,
                None,
            )
        }
        .map_err(|e| error("pipe_open_local_client", e))?;
        adopt(&mut self.pipe, raw, "pipe_open_local_client")?;
        // SAFETY: exact owned pipe; client handles initially default to byte-read mode.
        unsafe {
            SetNamedPipeHandleState(self.pipe.raw(), Some(&PIPE_READMODE_MESSAGE), None, None)
        }
        .map_err(|e| error("pipe_client_message_mode", e))?;
        self.run = Some(data.run);
        Ok(0)
    }
}
