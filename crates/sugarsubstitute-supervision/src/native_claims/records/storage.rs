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
    context: &sugarsubstitute_contracts::execution::JobContext,
) -> Result<Option<[u8; 128]>, ClaimError> {
    if file.account_sid.bytes()? != sid {
        return Err(ClaimError::invalid("claim_account_binding"));
    }
    let previous = match record_file::read_shape(file, context)? {
        record_file::RecordRead::Missing => None,
        record_file::RecordRead::Empty => return Err(ClaimError::invalid("empty_claim_record")),
        record_file::RecordRead::Present(bytes) => Some((Record::decode(&bytes)?, bytes)),
    };
    match previous {
        Some((record, bytes))
            if record.state == RecordState::Released && same_claim(&record, expected) =>
        {
            Ok(Some(bytes))
        }
        None if file.created => Ok(None),
        _ => Err(ClaimError::invalid("unresolved_claim_predecessor")),
    }
}

pub(crate) fn publish(
    file: &HeldFile,
    record: &Record,
    context: &sugarsubstitute_contracts::execution::JobContext,
) -> Result<(), ClaimError> {
    record_file::publish(
        file,
        &record.encode(),
        record_file::PublicationAuthority::Ordinary,
        context,
        |old| {
            if let record_file::RecordRead::Present(old) = old
                && !permits_replacement(&Record::decode(old)?, record)
            {
                return Err(ClaimError::invalid("changed_claim_generation"));
            }
            Ok(())
        },
    )
}
fn same_claim(old: &Record, new: &Record) -> bool {
    old.digest == new.digest && (old.resource == 0) == (new.resource == 0)
}
pub(crate) fn permits_replacement(old: &Record, new: &Record) -> bool {
    same_claim(old, new)
        && (old.state == RecordState::Released
            || (old.owner == new.owner
                && old.resource == new.resource
                && old.generation == new.generation))
}

/// Publish only under the retained election permission and its exact predecessor comparison.
/// # Errors
/// Preserves publication failures and rejects changed record or exclusion ownership.
pub(crate) fn publish_elected(
    file: &HeldFile,
    record: &Record,
    permission: &crate::claims::entry::ElectedCoordinatorPublication<'_>,
    context: &sugarsubstitute_contracts::execution::JobContext,
) -> Result<(), ClaimError> {
    record_file::publish(
        file,
        &record.encode(),
        record_file::PublicationAuthority::ElectedCoordinator(permission),
        context,
        |old| permission.validate(file, old),
    )
}
pub(crate) fn retire_coordinator(
    file: &HeldFile,
    expected: &mut Option<Record>,
    original_predecessor: Option<CoordinatorPredecessor<'_>>,
    context: &sugarsubstitute_contracts::execution::JobContext,
) -> Result<(), ClaimError> {
    let encoded = expected.as_ref().map(Record::encode);
    let observed = record_file::retire_publication(file, encoded.as_ref(), context)?;
    if prepare_coordinator_release(observed, expected, original_predecessor)? {
        publish(
            file,
            expected.as_ref().ok_or_else(|| ClaimError::invalid("missing_released_generation"))?,
            context,
        )?;
    }
    Ok(())
}
/// Borrow the exact predecessor established by an owning initialization policy.
#[derive(Clone, Copy)]
pub(crate) enum CoordinatorPredecessor<'a> {
    /// Exact compatible Released record consumed before ordinary initialization.
    Released(&'a [u8; 128]),
    /// Exact Held record retained under both election and coordinator exclusions.
    RecoveredHeld(&'a [u8; 128]),
}
/// Retain Released only after the sole record owner observed this exact generation.
/// Unactivated absence, empty shape and the exact original predecessor remain unchanged.
/// Recovered Held bytes are preserved only before actual new activation.
/// # Errors
/// Rejects corrupt or changed ownership without replacing the expected generation.
pub(crate) fn prepare_coordinator_release(
    observed: record_file::PublicationRetirement<128>,
    expected: &mut Option<Record>,
    original_predecessor: Option<CoordinatorPredecessor<'_>>,
) -> Result<bool, ClaimError> {
    let preserved = matches!(&observed, record_file::PublicationRetirement::Preserved(_));
    let observed = match observed {
        record_file::PublicationRetirement::Preserved(observed) => observed,
        record_file::PublicationRetirement::Activated(bytes) => {
            record_file::RecordRead::Present(bytes)
        }
    };
    let bytes = match observed {
        record_file::RecordRead::Missing | record_file::RecordRead::Empty => return Ok(false),
        record_file::RecordRead::Present(bytes) => bytes,
    };
    let actual = Record::decode(&bytes)?;
    if actual.resource == 0
        && match original_predecessor {
            Some(CoordinatorPredecessor::Released(original)) => {
                original == &bytes && actual.state == RecordState::Released
            }
            Some(CoordinatorPredecessor::RecoveredHeld(original)) => {
                preserved && original == &bytes && actual.state == RecordState::Held
            }
            None => false,
        }
    {
        return Ok(false);
    }
    let own =
        expected.as_ref().ok_or_else(|| ClaimError::invalid("missing_retirement_generation"))?;
    if actual.owner != own.owner
        || actual.resource != own.resource
        || actual.generation != own.generation
        || actual.digest != own.digest
        || actual.process != 0
        || actual.resource != 0
        || !matches!(actual.state, RecordState::Held | RecordState::Released)
    {
        return Err(ClaimError::invalid("changed_retirement_generation"));
    }
    let mut released = actual;
    released.state = RecordState::Released;
    let already_released = bytes == released.encode();
    *expected = Some(released);
    Ok(!already_released)
}

/// Settle native backend staging before proving a new Released generation.
/// # Errors
/// Retain custody and expected bytes on corrupt, foreign or changed generations.
pub(crate) fn retire_backend(
    file: &HeldFile,
    expected: &mut Option<Record>,
    original_released: Option<&[u8; 128]>,
    context: &sugarsubstitute_contracts::execution::JobContext,
) -> Result<(), ClaimError> {
    let encoded = expected.as_ref().map(Record::encode);
    let observed = record_file::retire_publication(file, encoded.as_ref(), context)?;
    if prepare_backend_release(observed, expected, original_released)? {
        publish(
            file,
            expected.as_ref().ok_or_else(|| ClaimError::invalid("backend_release_generation"))?,
            context,
        )?;
    }
    Ok(())
}
/// Preserve predecessor bytes until actual publication proves this backend's generation.
/// An acquisition rejected before adopting a generation retires only its own
/// unactivated outputs; preserved predecessor bytes confer no release authority.
/// # Errors
/// Reject unexpected activation and corrupt or changed owned generations.
pub(crate) fn prepare_backend_release(
    observed: record_file::PublicationRetirement<128>,
    expected: &mut Option<Record>,
    original_released: Option<&[u8; 128]>,
) -> Result<bool, ClaimError> {
    if expected.is_none() {
        return match observed {
            record_file::PublicationRetirement::Preserved(_) => Ok(false),
            record_file::PublicationRetirement::Activated(_) => {
                Err(ClaimError::invalid("backend_retirement_generation"))
            }
        };
    }
    let bytes = match observed {
        record_file::PublicationRetirement::Preserved(
            record_file::RecordRead::Missing | record_file::RecordRead::Empty,
        ) => return Ok(false),
        record_file::PublicationRetirement::Preserved(record_file::RecordRead::Present(bytes))
        | record_file::PublicationRetirement::Activated(bytes) => bytes,
    };
    let actual = Record::decode(&bytes)?;
    if original_released == Some(&bytes)
        && actual.state == RecordState::Released
        && actual.resource != 0
    {
        return Ok(false);
    }
    let own =
        expected.as_ref().ok_or_else(|| ClaimError::invalid("backend_retirement_generation"))?;
    if actual.owner != own.owner
        || actual.resource != own.resource
        || actual.resource == 0
        || actual.generation != own.generation
        || actual.digest != own.digest
        || actual.process != own.process
    {
        return Err(ClaimError::invalid("changed_backend_retirement_generation"));
    }
    let mut released = actual;
    released.state = RecordState::Released;
    released.process = 0;
    let needed = bytes != released.encode();
    *expected = Some(released);
    Ok(needed)
}
