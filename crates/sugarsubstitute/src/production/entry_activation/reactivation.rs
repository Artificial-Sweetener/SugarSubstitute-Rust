// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Coalesce repeated launch requests before acquiring another browser profile.
use super::*;
use sugarsubstitute_execution::windows_lifecycle_pipe::VerifiedLifecyclePeer;

/// A repeated entry either joins the existing window or waits for its presentation.
pub(super) enum Reuse {
    New,
    Waiting,
    Accepted,
    Expired,
}

/// Keep closing profiles on the existing successor-retirement path.
pub(super) fn reuse(
    records: &[Option<RetainedRecord>],
    pending: &PendingProjection,
    resources: &mut EntryResources<'_>,
) -> Result<Reuse> {
    if pending.restart {
        return Ok(Reuse::New);
    }
    let Some(JobOutcome::Success(value)) = pending.completion.as_ref().map(|done| &done.outcome)
    else {
        return Ok(Reuse::New);
    };
    if resources.client.now()? >= value.until
        && records.iter().flatten().any(|held| {
            held.record
                .profile_projection()
                .is_some_and(|old| old.requested_profile == value.requested_profile)
        })
    {
        tracing::info!(
            operation = "entry_repeat_expired",
            "Retired an expired repeated launch without changing the existing desktop"
        );
        return Ok(Reuse::Expired);
    }
    let existing = records.iter().flatten().map(|held| &held.record).find(|record| {
        !record.closed
            && record.failed.is_none()
            && record.pending_desktop_failure.is_none()
            && record.parent.as_ref().is_none_or(|parent| parent.normal_close_request().is_none())
            && record
                .profile_projection()
                .is_some_and(|old| old.requested_profile == value.requested_profile)
    });
    let Some(existing) = existing else { return Ok(Reuse::New) };
    restore_and_reply(
        existing.parent.as_ref().and_then(|parent| parent.activation_peer()),
        |peer| {
            sugarsubstitute_desktop::activate_desktop_process(
                peer.process_id(),
                peer.creation_time_100ns(),
            )
            .map_err(|cause| Box::new(cause) as Box<dyn Error>)
        },
        || {
            let until = resources
                .client
                .now()?
                .checked_add(std::time::Duration::from_secs(2))?
                .min(value.until);
            resources.intake.server().control.reply(
                value.entry_connection,
                value.entry_request,
                EntryOperationId::new(1)?,
                until,
                EntryOutcome::Accepted(existing.run),
            )?;
            Ok(())
        },
    )
}

/// Do not acknowledge an invisible owner before its native window has been restored.
fn restore_and_reply(
    peer: Option<VerifiedLifecyclePeer>,
    restore: impl FnOnce(VerifiedLifecyclePeer) -> Result<bool>,
    reply: impl FnOnce() -> Result<()>,
) -> Result<Reuse> {
    let Some(peer) = peer else { return Ok(Reuse::Waiting) };
    if !restore(peer)? {
        return Ok(Reuse::Waiting);
    }
    reply()?;
    Ok(Reuse::Accepted)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unpresented_owner_cannot_acknowledge_a_repeated_launch() -> Result<()> {
        let restored = std::cell::Cell::new(false);
        let replied = std::cell::Cell::new(false);
        assert!(matches!(
            restore_and_reply(
                None,
                |_| {
                    restored.set(true);
                    Ok(true)
                },
                || {
                    replied.set(true);
                    Ok(())
                }
            )?,
            Reuse::Waiting
        ));
        assert!(!restored.get() && !replied.get());
        Ok(())
    }
}
