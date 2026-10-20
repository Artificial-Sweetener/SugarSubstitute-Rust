// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Admit a backend successor only under both writer and independent Job-witness exclusions.
use super::*;
use crate::protected_storage::record_file::{self, RecordRead};
use sugarsubstitute_contracts::execution::JobContext;

/// Read the exact predecessor while the caller retains both independently owned locks.
/// A live witness prevents acquisition before this function. Once both owners are gone,
/// the kernel closes their kill-on-close Job and a witnessed record can be replaced.
/// Legacy active records carry no such proof and remain untouched.
pub(super) fn predecessor(
    writer: &HeldFile,
    witness: &HeldFile,
    sid: &[u8],
    expected: &Record,
    context: &JobContext,
) -> Result<Option<[u8; 128]>, ClaimError> {
    if !writer.is_locked()
        || !witness.is_locked()
        || writer.account_sid.bytes()? != sid
        || witness.account_sid.bytes()? != sid
        || writer.record_path.path()? != witness.record_path.path()?
    {
        return Err(ClaimError::invalid("backend_recovery_exclusions"));
    }
    match record_file::read_shape(writer, context)? {
        RecordRead::Missing if writer.created => Ok(None),
        RecordRead::Present(bytes) => {
            let old = Record::decode(&bytes)?;
            if !admits(&old, expected) {
                return Err(ClaimError::invalid("unresolved_claim_predecessor"));
            }
            if old.state != RecordState::Released {
                tracing::info!(operation="backend_abandoned_recovered", previous_owner=?old.owner,
                    "Recovered abandoned backend under writer and witness exclusions");
            }
            Ok(Some(bytes))
        }
        _ => Err(ClaimError::invalid("unresolved_claim_predecessor")),
    }
}

/// Preserve original bytes on failure and atomically replace only the checked predecessor.
pub(super) fn publish(
    file: &HeldFile,
    record: &Record,
    predecessor: Option<&[u8; 128]>,
    context: &JobContext,
) -> Result<(), ClaimError> {
    record_file::publish(
        file,
        &record.encode(),
        record_file::PublicationAuthority::Ordinary,
        context,
        |old| match (old, predecessor) {
            (RecordRead::Missing, None) => Ok(()),
            (RecordRead::Present(actual), Some(expected)) if actual == expected => Ok(()),
            _ => Err(ClaimError::invalid("changed_backend_predecessor")),
        },
    )
}

/// This predicate is usable only while both native exclusions above are held.
fn admits(old: &Record, next: &Record) -> bool {
    (old.state == RecordState::Released && records::storage::permits_replacement(old, next))
        || (old.version == 2
            && next.version == 2
            && old.resource != 0
            && next.resource != 0
            && old.digest == next.digest)
}

#[cfg(test)]
mod tests;
