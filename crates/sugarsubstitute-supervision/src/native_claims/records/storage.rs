// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Claims alone decide schema-one predecessor and replacement compatibility.
use super::*;
use crate::protected_storage::record_file;
#[cfg(test)]
use record_file::finish_activation;
#[cfg(test)]
mod tests;

pub(crate) fn check_predecessor(
    file: &HeldFile,
    sid: &[u8],
    expected: &Record,
) -> Result<(), ClaimError> {
    if file.account_sid != sid {
        return Err(ClaimError::invalid("claim_account_binding"));
    }
    let previous = match record_file::read_shape(file)? {
        record_file::RecordRead::Missing => None,
        record_file::RecordRead::Empty => return Err(ClaimError::invalid("empty_claim_record")),
        record_file::RecordRead::Present(bytes) => Some(Record::decode(&bytes)?),
    };
    match previous {
        Some(record) if record.state == RecordState::Released && same_claim(&record, expected) => {
            Ok(())
        }
        None if file.created => Ok(()),
        _ => Err(ClaimError::invalid("unresolved_claim_predecessor")),
    }
}

pub(crate) fn publish(file: &HeldFile, record: &Record) -> Result<(), ClaimError> {
    record_file::publish(file, &record.encode(), |old| {
        if let record_file::RecordRead::Present(old) = old
            && !permits_replacement(&Record::decode(old)?, record)
        {
            return Err(ClaimError::invalid("changed_claim_generation"));
        }
        Ok(())
    })
}
fn same_claim(old: &Record, new: &Record) -> bool {
    old.digest == new.digest && (old.resource == 0) == (new.resource == 0)
}
fn permits_replacement(old: &Record, new: &Record) -> bool {
    same_claim(old, new)
        && (old.state == RecordState::Released
            || (old.owner == new.owner
                && old.resource == new.resource
                && old.generation == new.generation))
}
