// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Hand startup to the engine of the generation that should run, so every handoff stays inside
//! one generation.
//!
//! The generation that should run is the candidate while one is prepared or on trial, otherwise
//! the accepted one. An engine started with hop 0 that is not that generation verifies the
//! target generation's payload, points the stable entry at its engine, starts it with hop 1 and
//! exits once it takes over. An engine at hop 1 never redirects: it is the root's fallback, or it
//! was redirected to. If the redirect target cannot take over, or this engine is the fallback,
//! it starts only its own accepted release, never a trial.
use super::*;
use crate::{
    application_launch::{ENGINE_TAKEOVER, MAX_ENGINE_HOPS, Takeover, start_engine},
    setup_storage::{application::VerifiedApplication, launch_pointer},
};
use std::fmt;
use sugarsubstitute_contracts::installation::{Generation, GenerationId};

/// Reserve beside the selection store on the admitted worker; covers payload verification of
/// one generation and one engine start.
pub const ROUTE_RESERVATION_BYTES: u64 = super::super::LAUNCH_RESERVATION_BYTES
    + launch_pointer::POINTER_RESERVATION_BYTES
    + crate::application_launch::CREATION_RESERVATION_BYTES;

/// What this engine does with startup.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Route {
    /// This generation should run; continue with ordinary startup, trial included.
    Own,
    /// Another generation's engine took over startup; exit.
    Redirected,
    /// Start only this generation's accepted release; no trial runs.
    AcceptedOnly,
}

/// Why this engine cannot start this installation at all.
#[derive(Debug)]
pub enum RouteFailure {
    /// The installation has no accepted release; setup must repair it.
    NeedsInstallation,
    /// This generation is neither the one that should run nor an accepted release, and it may
    /// not redirect.
    Foreign,
    /// Selection storage failed or rejected its record.
    Selection(SelectionFailure),
}
impl fmt::Display for RouteFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NeedsInstallation => f.write_str("The installation has no accepted release"),
            Self::Foreign => f.write_str("This release is not selected to start the installation"),
            Self::Selection(cause) => write!(f, "{cause}"),
        }
    }
}
impl std::error::Error for RouteFailure {}
impl From<SelectionFailure> for RouteFailure {
    fn from(cause: SelectionFailure) -> Self {
        Self::Selection(cause)
    }
}
impl From<io::Error> for RouteFailure {
    fn from(cause: io::Error) -> Self {
        Self::Selection(SelectionFailure::Storage(cause))
    }
}

/// Decide, and if needed perform, the handoff of startup for the engine of generation `own`.
/// Run on an admitted blocking infrastructure job reserving `ROUTE_RESERVATION_BYTES`.
/// # Errors
/// `NeedsInstallation`, `Foreign` and selection failures. A redirect target that cannot take
/// over is not an error: this engine continues with `AcceptedOnly` when it is accepted.
pub fn route_launch(
    installation: &Path,
    own: &GenerationId,
    hops: u8,
    context: &JobContext,
) -> Result<Route, RouteFailure> {
    let store = SelectionStore::open(installation, context)?;
    let record = store.load(context)?.ok_or(SelectionFailure::Missing)?;
    match decide(&record, own, hops)? {
        Decision::Run(route) => Ok(route),
        Decision::Redirect { target, fallback } => {
            match store.redirect(target, fallback, context) {
                Ok(()) => return Ok(Route::Redirected),
                Err(cause) => tracing::warn!(
                    operation = "launch_engine_redirect_failed",
                    installation = %record.installation().as_str(),
                    target = target.id.as_str(),
                    cause = %cause,
                    "The selected release's launch engine did not take over"
                ),
            }
            match record.accepted() {
                Some(accepted) if &accepted.id == own => Ok(Route::AcceptedOnly),
                _ => Err(RouteFailure::Foreign),
            }
        }
    }
}

/// The routing decision for one record, before any storage or process work.
#[derive(Debug, Eq, PartialEq)]
enum Decision<'a> {
    Run(Route),
    Redirect { target: &'a Generation, fallback: &'a GenerationId },
}

/// The generation that should run is a prepared or trial candidate, otherwise the accepted one.
/// A staging candidate is incomplete and never runs.
fn decide<'a>(
    record: &'a InstallationRecord,
    own: &'a GenerationId,
    hops: u8,
) -> Result<Decision<'a>, RouteFailure> {
    let accepted = record.accepted().ok_or(RouteFailure::NeedsInstallation)?;
    let target = match record.candidate() {
        Some(candidate @ (Candidate::Prepared { .. } | Candidate::Trial { .. })) => {
            candidate.generation()
        }
        _ => accepted,
    };
    if &target.id == own {
        return Ok(Decision::Run(Route::Own));
    }
    if hops < MAX_ENGINE_HOPS {
        let fallback = if accepted.id == target.id { own } else { &accepted.id };
        return Ok(Decision::Redirect { target, fallback });
    }
    if &accepted.id == own {
        return Ok(Decision::Run(Route::AcceptedOnly));
    }
    Err(RouteFailure::Foreign)
}

impl SelectionStore {
    /// Verify `target`'s payload, point the stable entry at its engine with this installation's
    /// accepted release as fallback, start it with the last hop and wait for it to take over.
    fn redirect(
        &self,
        target: &Generation,
        fallback_generation: &GenerationId,
        context: &JobContext,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let application = VerifiedApplication::open(&self._installation, target.clone(), context)?;
        let primary =
            launch_pointer::EngineTarget::of(&target.id, &application.directory, context)?;
        let fallback_directory = Directory::open(
            &self._installation.path().join("releases").join(fallback_generation.as_str()),
            false,
        )?;
        let fallback =
            launch_pointer::EngineTarget::of(fallback_generation, &fallback_directory, context)?;
        let pointer = launch_pointer::LaunchPointer::new(primary.clone(), Some(fallback));
        self.protected(context, |protection| {
            let _writer = writer(&self.root, protection, context)?;
            let state =
                Directory::open(&super::super::ownership::state_path(&self._installation), false)?;
            launch_pointer::publish(&state, &pointer, protection, context)?;
            Ok(())
        })?;
        let engine = launch_pointer::pin_engine(self._installation.path(), &primary, context)?;
        drop(application);
        let started = start_engine(&engine, self._installation.path(), MAX_ENGINE_HOPS, context)?;
        drop(engine);
        let until = context.now().checked_add(ENGINE_TAKEOVER)?;
        match started.await_takeover(context, until)? {
            Takeover::TookOver => Ok(()),
            Takeover::Exited => {
                Err("The selected release's engine exited before taking over".into())
            }
            Takeover::Silent => Err("The selected release's engine stayed silent".into()),
        }
    }
}

#[cfg(test)]
mod tests;
