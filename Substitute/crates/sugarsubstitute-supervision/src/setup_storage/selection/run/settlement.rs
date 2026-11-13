// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Resolve a settled trial under its original, continuously retained native run custody.
use super::*;
use sugarsubstitute_contracts::installation::TrialVerdict;

impl InstallationRun {
    /// Finish an owned attempt with the verdict its observer classified. Preserve committed
    /// acceptance even if its worker completion was lost, and any earlier settlement of the same
    /// attempt: the first published verdict stands, and repeated finalization by the coordinator
    /// and its custody witness keeps later preparation intact. A bad verdict rolls back and
    /// counts a strike; no verdict returns the candidate to prepared for the next launch.
    /// This requires the original live token, not a lock reacquired after a crash.
    /// # Errors
    /// Rejects foreign custody, changed activation history, cancellation and native failures.
    pub fn finish_trial(
        &self,
        trial: &OwnedTrial,
        verdict: TrialVerdict,
        context: &JobContext,
    ) -> Result<InstallationRecord, SelectionFailure> {
        self.check_trial(trial)?;
        let store = &self.custody.store;
        through_contention(context, || {
            store.protected(context, |protection| {
                let _writer = super::super::writer(&store.root, protection, context)?;
                let current = store.read(protection)?.ok_or(SelectionFailure::Missing)?;
                let resolutions = resolutions(trial, &current)?;
                if current == trial.snapshot {
                    let settled = match verdict {
                        TrialVerdict::Bad(_) => &resolutions[1],
                        TrialVerdict::NoVerdict(_) => &resolutions[2],
                    };
                    store.publish(protection, settled, context)?;
                    tracing::info!(
                        operation = "installation_trial_settled",
                        verdict = ?verdict,
                        installation = trial.attempt.installation.as_str(),
                        generation = trial.attempt.generation.id.as_str(),
                        revision = settled.revision(),
                        "Settled an unaccepted installation trial"
                    );
                    return Ok(settled.clone());
                }
                if resolutions.iter().any(|resolved| preparation_successor(&current, resolved)) {
                    return Ok(current);
                }
                Err(SelectionFailure::Changed)
            })
        })
    }
}

/// Repeat `publish` while another process holds the selection lock, until it succeeds, fails for
/// another reason, or `context` is cancelled or expires. Each attempt waits for the lock in the
/// kernel for up to `short_lock::WAIT`, so contention never spins. A trial's verdict must not be
/// lost to a second launch briefly reading the record.
pub(super) fn through_contention<T>(
    context: &JobContext,
    mut publish: impl FnMut() -> Result<T, SelectionFailure>,
) -> Result<T, SelectionFailure> {
    loop {
        match publish() {
            Err(SelectionFailure::LockTimeout) if crate::setup_storage::check(context).is_ok() => {
                tracing::warn!(
                    operation = "installation_trial_publication_contended",
                    "Another process still holds the installation selection; waiting again"
                );
            }
            other => return other,
        }
    }
}

/// Every way this attempt can have been settled: accepted, judged bad, or released without a
/// verdict. A bad verdict already published keeps its own strike time, so a repeated settlement
/// recognizes it however much later it runs.
fn resolutions(
    trial: &OwnedTrial,
    current: &InstallationRecord,
) -> Result<[InstallationRecord; 3], SelectionFailure> {
    let published = current
        .strikes()
        .iter()
        .find(|strike| strike.package == trial.attempt.generation.package)
        .map(|strike| strike.at)
        .filter(|_| current != &trial.snapshot);
    let at = match published {
        Some(at) => at,
        None => unix_now()?,
    };
    Ok([
        trial.snapshot.accept(&trial.attempt)?,
        trial.snapshot.reject(&trial.attempt, at)?,
        trial.snapshot.release_trial(&trial.attempt)?,
    ])
}

/// UTC Unix seconds that date strikes and decide whether they still withhold a package. Strikes
/// only expire, so a wrong clock at worst gives a release one more trial or withholds it for at
/// most one strike lifetime.
pub(in crate::setup_storage) fn unix_now() -> Result<u64, SelectionFailure> {
    let elapsed = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(std::io::Error::other)?;
    Ok(elapsed.as_secs())
}

/// Ordinary preparation may advance while this run still owns the same resolved activation.
fn preparation_successor(current: &InstallationRecord, resolved: &InstallationRecord) -> bool {
    if current.revision() == resolved.revision() {
        return current == resolved;
    }
    current.installation() == resolved.installation()
        && current.revision() > resolved.revision()
        && current.accepted() == resolved.accepted()
        && current.retained() == resolved.retained()
        && current.strikes() == resolved.strikes()
        && current.rollback() == resolved.rollback()
        && current.repair() == resolved.repair()
        && !matches!(current.candidate(), Some(Candidate::Trial { .. }))
}

#[cfg(test)]
mod tests;
