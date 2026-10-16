// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Encode original Root startup bytes directly into their actual charged session buffer.
use super::*;
use crate::{RegisteredRecoveryGrant, scope::lock};
use sugarsubstitute_contracts::{incident::DurableAcknowledgement, recovery_supervisor::wire::*};
impl RecoveryPipeControl {
    /// Match the genuine grant and actual same-session outgoing allocation before copying.
    fn original_sender(
        &self,
        grant: &RegisteredRecoveryGrant,
        buffer: &LifecyclePipeWriteBuffer,
    ) -> Result<(), RecoveryStartupError> {
        self.validate_current_admission().map_err(RecoveryStartupError::Transport)?;
        grant.check().map_err(RecoveryStartupError::Admission)?;
        if self.inner.shared.child
            || !buffer.shared.same_allocation(&self.inner.shared)
            || self
                .inner
                .shared
                .recovery_grant
                .as_ref()
                .is_none_or(|source| !source.same_grant(grant))
            || self.prepared_key() != Some(grant.key())
            || !lock(&grant.state().map_err(RecoveryStartupError::Admission)?.stages).committed
        {
            return Err(RecoveryStartupError::Binding);
        }
        Ok(())
    }
    /// Copy one bounded fragment directly from the actual original registered archive.
    /// Returning bytes/next offset is encoding only; the caller advances its queue
    /// after actual complete native Written, under the original finite cutoff.
    /// No child-provided bytes, paths or environment replace the Root source.
    /// # Errors
    /// Rejects a foreign grant/buffer, stopped/uncommitted original session, replayed
    /// source, invalid offset, source expiry or complete native message bound.
    pub fn encode_original_startup(
        &self,
        grant: &RegisteredRecoveryGrant,
        sequence: u64,
        offset: usize,
        buffer: &mut LifecyclePipeWriteBuffer,
    ) -> Result<(usize, usize), RecoveryStartupError> {
        self.original_sender(grant, buffer)?;
        let body = grant.state().map_err(RecoveryStartupError::Admission)?.startup_body();
        if offset >= body.len() || body.len() > RECOVERY_STARTUP_BYTES {
            return Err(RecoveryStartupError::Order);
        }
        let end = body.len().min(offset + RECOVERY_STARTUP_CHUNK);
        let frame = RecoveryFrame {
            key: grant.key(),
            sequence,
            command: RecoveryCommand::Startup {
                total: body.len() as u32,
                offset: offset as u32,
                bytes: &body[offset..end],
            },
        };
        let bytes = encode(&frame, RecoveryReceiver::Supervisor, buffer.as_mut_bytes())
            .map_err(RecoveryStartupError::Wire)?;
        Ok((bytes, end))
    }
    /// Encode exact Root-assigned launch/navigation facts for the genuine granted run.
    /// # Errors
    /// Rejects foreign source/buffer, wrong run, stopped admission or invalid wire shape.
    pub fn encode_original_desktop_identity(
        &self,
        grant: &RegisteredRecoveryGrant,
        sequence: u64,
        identity: sugarsubstitute_contracts::startup::MainIdentity,
        buffer: &mut LifecyclePipeWriteBuffer,
    ) -> Result<usize, RecoveryStartupError> {
        self.original_sender(grant, buffer)?;
        if identity.run != grant.key().run() {
            return Err(RecoveryStartupError::Binding);
        }
        encode(
            &RecoveryFrame {
                key: grant.key(),
                sequence,
                command: RecoveryCommand::DesktopIdentity {
                    launch: identity.launch,
                    navigation: identity.navigation,
                },
            },
            RecoveryReceiver::Supervisor,
            buffer.as_mut_bytes(),
        )
        .map_err(RecoveryStartupError::Wire)
    }
    /// Encode a bounded fragment of Root's configured capture location and actual pin identity.
    /// These bytes grant no filesystem authority. The fresh process verifies actual account,
    /// canonical directory/reparse chain and exact pinned identity independently.
    /// # Errors
    /// Rejects foreign source/buffer, stopped admission, malformed bounds or wire shape.
    pub fn encode_original_capture_root(
        &self,
        grant: &RegisteredRecoveryGrant,
        sequence: u64,
        identity: [u8; 24],
        units_le: &[u8],
        offset: usize,
        buffer: &mut LifecyclePipeWriteBuffer,
    ) -> Result<(usize, usize), RecoveryStartupError> {
        self.original_sender(grant, buffer)?;
        if offset >= units_le.len()
            || units_le.len() > RECOVERY_ROOT_BYTES
            || !offset.is_multiple_of(2)
        {
            return Err(RecoveryStartupError::Order);
        }
        let end = units_le.len().min(offset + RECOVERY_ROOT_CHUNK);
        let bytes = encode(
            &RecoveryFrame {
                key: grant.key(),
                sequence,
                command: RecoveryCommand::CaptureRoot {
                    identity,
                    total: units_le.len() as u32,
                    offset: offset as u32,
                    units_le: &units_le[offset..end],
                },
            },
            RecoveryReceiver::Supervisor,
            buffer.as_mut_bytes(),
        )
        .map_err(RecoveryStartupError::Wire)?;
        Ok((bytes, end))
    }
    /// Encode Root's configured path directly, retaining no independent UTF16 copy.
    /// The actual fresh registration independently verifies its protected directory identity.
    /// # Errors
    /// Rejects foreign/stopped grant or buffer, path units/bounds, offset or wire shape.
    #[cfg(windows)]
    pub fn encode_original_capture_root_path(
        &self,
        grant: &RegisteredRecoveryGrant,
        sequence: u64,
        identity: [u8; 24],
        root: &std::path::Path,
        offset_units: usize,
        buffer: &mut LifecyclePipeWriteBuffer,
    ) -> Result<(usize, usize), RecoveryStartupError> {
        use std::os::windows::ffi::OsStrExt;
        self.original_sender(grant, buffer)?;
        let total = root.as_os_str().encode_wide().count();
        let total = u32::try_from(total).map_err(|_| RecoveryStartupError::Order)?;
        let offset = u32::try_from(offset_units).map_err(|_| RecoveryStartupError::Order)?;
        let count = (total as usize).saturating_sub(offset_units).min(RECOVERY_ROOT_CHUNK / 2);
        let units = root.as_os_str().encode_wide().skip(offset_units).take(count);
        encode_capture_root_units(
            grant.key(),
            sequence,
            identity,
            total,
            offset,
            units,
            buffer.as_mut_bytes(),
        )
        .map(|(bytes, next)| (bytes, next as usize))
        .map_err(RecoveryStartupError::Wire)
    }
    /// Encode the exact Root-owned durable ACK after all original startup bytes were written.
    /// Receiver order independently rejects early/repeated commit. This cannot grant
    /// durability: the composing Root holds the genuine committed storage response.
    /// # Errors
    /// Rejects foreign/stopped/uncommitted source, changed report/revision, sequence or buffer.
    pub fn encode_original_commit(
        &self,
        grant: &RegisteredRecoveryGrant,
        sequence: u64,
        acknowledgement: DurableAcknowledgement,
        buffer: &mut LifecyclePipeWriteBuffer,
    ) -> Result<usize, RecoveryStartupError> {
        self.original_sender(grant, buffer)?;
        encode(
            &RecoveryFrame {
                key: grant.key(),
                sequence,
                command: RecoveryCommand::Commit(acknowledgement),
            },
            RecoveryReceiver::Supervisor,
            buffer.as_mut_bytes(),
        )
        .map_err(RecoveryStartupError::Wire)
    }
}
