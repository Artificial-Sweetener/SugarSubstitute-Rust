// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Record what a close did not keep, so the next start can tell the user once.
//!
//! A close whose outcome is not fully saved writes `close-save.json` in the profile directory
//! (`user\sessions\<profile>`) before the exit intent. The next start reads it on its workspace
//! load worker, turns it into a [`NotSavedLastTime`] notice and marks the record notified in
//! place, so the notice appears exactly once and the record itself stays as evidence. A record
//! this build cannot read is kept untouched and reported as an error for the caller to log.
use super::*;
use std::io::Read;
use sugarsubstitute_application::workspace_session::close_save::{
    CloseReason, CloseSaveOutcome, NotSavedLastTime, SaveFailure, SaveVerdict, UnsavedCause,
    UnsavedItem,
};
use sugarsubstitute_contracts::execution::{JobContext, Moment};
use sugarsubstitute_supervision::{
    ArtifactError, ArtifactPublication, with_artifact_input, write_artifact,
};

/// The record's leaf inside the profile directory.
pub const CLOSE_RECORD: &str = "close-save.json";
const FORMAT: &str = "sugarsubstitute.close-save";
const VERSION: u32 = 1;
const MAX_RECORD_BYTES: u64 = 4 * 1024;

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct Record {
    format: String,
    version: u32,
    notified: bool,
    reason: String,
    workspace: Entry,
    inputs: Entry,
}

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase", tag = "verdict")]
enum Entry {
    Saved,
    Stale { unsaved_for_ms: u64 },
    Failed { failure: String },
}

impl Entry {
    fn new(verdict: SaveVerdict, ended: Moment) -> Self {
        match verdict {
            SaveVerdict::Saved => Self::Saved,
            SaveVerdict::Stale { since } => Self::Stale {
                unsaved_for_ms: ended
                    .checked_duration_since(since)
                    .unwrap_or_default()
                    .as_millis()
                    .try_into()
                    .unwrap_or(u64::MAX),
            },
            SaveVerdict::Failed(failure) => Self::Failed { failure: failure.as_str().into() },
        }
    }

    fn cause(&self) -> Result<Option<UnsavedCause>, StoreError> {
        Ok(match self {
            Self::Saved => None,
            Self::Stale { unsaved_for_ms } => Some(UnsavedCause::Interrupted {
                unsaved_for: std::time::Duration::from_millis(*unsaved_for_ms),
            }),
            Self::Failed { failure } => {
                Some(UnsavedCause::Failed(SaveFailure::parse(failure).ok_or_else(invalid)?))
            }
        })
    }
}

fn invalid() -> StoreError {
    StoreError::Storage(io::Error::new(io::ErrorKind::InvalidData, "Unsupported close-save record"))
}

/// Durably record `outcome`, which ended at `ended` on the execution clock, in `profile`.
/// Runs on an execution worker before the exit intent.
/// # Errors
/// Preserves storage, cancellation and deadline failures; the caller reports them.
pub fn write_worker(
    profile: &Path,
    outcome: &CloseSaveOutcome,
    ended: Moment,
    context: &JobContext,
) -> Result<(), StoreError> {
    let record = Record {
        format: FORMAT.into(),
        version: VERSION,
        notified: false,
        reason: outcome.reason.as_str().into(),
        workspace: Entry::new(outcome.workspace, ended),
        inputs: Entry::new(outcome.inputs, ended),
    };
    publish(profile, &record, context)
}

/// Read the previous close's record and return its notice once: an unnotified record of
/// lost work yields the notice and is marked notified in place.
/// # Errors
/// Refuses an unreadable or unsupported record, which stays untouched, and preserves
/// storage failures.
pub fn take_notice_worker(
    profile: &Path,
    context: &JobContext,
) -> Result<Option<NotSavedLastTime>, StoreError> {
    let bytes =
        match with_artifact_input(&profile.join(CLOSE_RECORD), MAX_RECORD_BYTES, context, |input| {
            let mut bytes = Vec::new();
            input.read_to_end(&mut bytes)?;
            Ok::<_, io::Error>(bytes)
        }) {
            Ok(bytes) => bytes,
            Err(ArtifactError::Storage(cause)) if cause.kind() == io::ErrorKind::NotFound => {
                return Ok(None);
            }
            Err(cause) => return Err(files::artifact(cause)),
        };
    let mut record: Record = serde_json::from_slice(&bytes).map_err(|_| invalid())?;
    if record.format != FORMAT || record.version != VERSION {
        return Err(invalid());
    }
    let reason = CloseReason::parse(&record.reason).ok_or_else(invalid)?;
    let items: Vec<_> = [
        record.workspace.cause()?.map(UnsavedItem::Workspace),
        record.inputs.cause()?.map(UnsavedItem::InputCanvas),
    ]
    .into_iter()
    .flatten()
    .collect();
    if record.notified || items.is_empty() {
        return Ok(None);
    }
    record.notified = true;
    publish(profile, &record, context)?;
    Ok(Some(NotSavedLastTime { reason, items }))
}

fn publish(profile: &Path, record: &Record, context: &JobContext) -> Result<(), StoreError> {
    let bytes = serde_json::to_vec(record).map_err(io::Error::other)?;
    write_artifact(
        &profile.join(CLOSE_RECORD),
        true,
        ArtifactPublication::ReplaceSelected,
        bytes.len().max(1),
        context,
        |output| output.write_all(&bytes),
    )
    .map_err(files::artifact)
}
