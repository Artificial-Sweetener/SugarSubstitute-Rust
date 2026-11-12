// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain each published trial independently of cancellable execution-result delivery.
use super::*;
use sugarsubstitute_contracts::execution::JobContext;
use sugarsubstitute_supervision::{
    SettledUpdaterProbe, WindowsLaunchInput,
    setup_storage::{
        PreparationImage, PreparedUpdater, UnstartedUpdaterTrial, UpdaterProbeTransfer,
    },
};

/// Unique storage custody; only `Input` can transfer to the contained process owner.
pub(super) enum Phase {
    Prepared(Option<Box<PreparedUpdater>>),
    Trial(Option<UnstartedUpdaterTrial>),
    Probe(Option<UpdaterProbeTransfer>),
    Input(Option<WindowsLaunchInput>),
    Settled(Box<SettledUpdaterProbe>),
    Image(Option<Box<PreparationImage>>),
    Empty,
}

/// Mutated on one admitted worker at a time, observed only after its actual terminal outcome.
pub(super) struct State {
    pub phase: Phase,
    pub failure: Option<Arc<io::Error>>,
}
impl State {
    /// Every fallible published-custody transition preserves its original owner on failure.
    pub(super) fn advance(&mut self, key: CoordinatorKey, context: &JobContext) -> io::Result<()> {
        let next = match &mut self.phase {
            Phase::Prepared(prepared) => Phase::Trial(Some(
                prepared
                    .take()
                    .ok_or_else(|| absent("Prepared engine"))?
                    .reserve_trial(context)
                    .map_err(io::Error::other)?,
            )),
            Phase::Trial(trial) => {
                let original = trial.take().ok_or_else(|| absent("Reserved engine trial"))?;
                let receipt = crate::authority_clock::Receipt::default();
                match original.into_probe(|| receipt.now(), context) {
                    Ok(probe) => {
                        // Retain even a completed publication before reporting a failed clock.
                        self.phase = Phase::Probe(Some(probe));
                        return receipt.take_failure().map_or(Ok(()), Err);
                    }
                    Err(rejected) => {
                        let (original, cause) = rejected.into_parts();
                        *trial = Some(original);
                        return Err(receipt
                            .take_failure()
                            .unwrap_or_else(|| io::Error::other(cause)));
                    }
                }
            }
            Phase::Probe(probe) => {
                let environment = crate::worker_environment::capture().map_err(io::Error::other)?;
                Phase::Input(Some(WindowsLaunchInput::updater_probe(key, probe, environment)?))
            }
            Phase::Settled(probe) => {
                crate::authority_clock::authorize(|now| probe.publish(now, context))?;
                Phase::Image(Some(Box::new(
                    probe.pin_preparation(context).map_err(io::Error::other)?,
                )))
            }
            Phase::Image(image) => {
                let environment = crate::worker_environment::capture().map_err(io::Error::other)?;
                let image = image.take().ok_or_else(|| absent("Accepted engine image"))?;
                let workspace = image.installation().join(".update-workspace");
                Phase::Input(Some(WindowsLaunchInput::preparation(
                    key,
                    *image,
                    &workspace,
                    environment,
                )?))
            }
            Phase::Input(_) | Phase::Empty => return Err(absent("Engine storage step")),
        };
        self.phase = next;
        Ok(())
    }

    /// Explicitly retire original authority on a live cleanup job; failed storage stays retryable.
    pub(super) fn retire(&mut self, context: &JobContext) -> io::Result<()> {
        match &mut self.phase {
            Phase::Trial(Some(trial)) => trial.restore(context).map_err(io::Error::other)?,
            Phase::Probe(Some(probe)) => {
                probe.withdraw_before_launch(context).map_err(io::Error::other)?
            }
            Phase::Input(Some(input)) if input.is_updater_probe() => {
                input.withdraw_updater_probe(context).map_err(io::Error::other)?;
            }
            Phase::Settled(probe) => {
                crate::authority_clock::authorize(|now| probe.publish(now, context))?;
            }
            _ => {}
        }
        self.phase = Phase::Empty;
        Ok(())
    }

    /// Additional memory only; the original authenticated source retains its existing admission.
    pub(super) fn reservation(&self, closing: bool) -> u64 {
        use sugarsubstitute_supervision::setup_storage::{
            RESERVATION_BYTES, TRUST_OPERATION_RESERVATION_BYTES,
        };
        TRUST_OPERATION_RESERVATION_BYTES
            + if !closing && matches!(self.phase, Phase::Settled(_)) {
                RESERVATION_BYTES
            } else {
                0
            }
    }
}

/// A missing unique owner is an invariant failure, never a successful updater result.
fn absent(owner: &str) -> io::Error {
    io::Error::other(format!("{owner} custody absent"))
}
