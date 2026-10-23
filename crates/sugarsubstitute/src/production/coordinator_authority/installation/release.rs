// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Release coordinator installation custody on the existing cleanup executor before it joins.
use super::*;
use std::io;
use sugarsubstitute_execution::BlockingInputError;

/// Native file guards and their original allocation remain together until worker destruction.
struct Custody {
    _completion: Option<Completion>,
    _inherited: Option<setup_storage::InheritedLaunch>,
}

/// One admitted destruction and its observed outcome; rejection preserves caller custody.
pub(super) struct Release {
    job: Option<OwnedJob<(), io::Error>>,
    terminal: Option<terminal::Terminal>,
    admission: Option<BlockingInputError>,
    confirmed: bool,
}
impl Release {
    /// Construction neither releases resources nor grants permission to do so.
    pub(super) fn new() -> Self {
        Self { job: None, terminal: None, admission: None, confirmed: false }
    }

    /// The returned typed completion belongs to this retained handle; no cached phase proves exit.
    fn observe(&mut self) -> Result<bool, AuthorityIssue> {
        if let Some(job) = &mut self.job
            && let Some(done) = job.try_take_completion()
        {
            let valid = done.scope_id == job.scope_id() && done.spec == job.spec();
            self.confirmed = valid && done.outcome.kind() == TerminalKind::Success;
            self.terminal = Some(terminal::Terminal::observe(done));
            self.job = None;
            if !valid {
                return Err(AuthorityIssue::Identity);
            }
        }
        if self.admission.is_some() {
            return Err(AuthorityIssue::Adapter);
        }
        match self.terminal.as_ref().map(terminal::Terminal::kind) {
            Some(TerminalKind::Success) if self.confirmed => Ok(true),
            Some(_) => Err(AuthorityIssue::Stopped),
            None => Ok(false),
        }
    }

    /// Preserve the original adapter/storage diagnostic after observed teardown failure.
    fn failure_source(&self) -> Option<&(dyn Error + 'static)> {
        self.admission
            .as_ref()
            .map(|cause| cause as &(dyn Error + 'static))
            .or_else(|| self.terminal.as_ref().and_then(terminal::Terminal::failure_source))
    }
}

impl CoordinatorAuthority {
    /// Release installation run ownership only after child processes, claims and publication
    /// writers have actually settled. Call before joining the same executor, using its cleanup
    /// scope and original shutdown cutoff. This does not retire any independent helper image.
    /// Failure retains diagnostics and any unadmitted custody; it never implies release.
    pub(crate) fn retire_installation(
        &mut self,
        client: &ExecutionClient,
        scope: &Scope,
        jobs: &JobSequence,
    ) -> Result<bool, AuthorityIssue> {
        if !self.settled() {
            return Err(AuthorityIssue::Pending);
        }
        if self.installation.release.observe()? {
            return Ok(true);
        }
        if self.installation.release.job.is_some() {
            return Ok(false);
        }
        let successful = self.installation.ready();
        if !successful && self.installation.inherited.is_none() {
            return Ok(true);
        }
        let until = self.close_until.ok_or(AuthorityIssue::Pending)?;
        let now = client.now().map_err(|cause| {
            self.fail(Failure::Clock(cause));
            AuthorityIssue::Clock
        })?;
        if self.association.is_some_and(|(owner, _)| owner != scope.owner_id()) {
            return Err(AuthorityIssue::Identity);
        }
        if now >= until {
            return Err(AuthorityIssue::Closed);
        }
        // Forwarded launches never acquire election authority. Destruction needs only
        // the owned guards and real cleanup admission, not permission to acquire claims.
        let spec = JobSpec {
            id: jobs.allocate().map_err(|cause| {
                self.fail(Failure::Identity(cause));
                AuthorityIssue::Identity
            })?,
            operation: Operation::NativeLifecycle,
            class: JobClass::InfrastructureLifecycle,
            deadline: Some(until),
            reservation_bytes: 4096,
            replacement: None,
        };
        let mut input = Some(Custody {
            _completion: if successful { self.installation.completion.take() } else { None },
            _inherited: self.installation.inherited.take(),
        });
        match client.submit_blocking_owned_input(scope, spec, &mut input, |custody, _context| {
            drop(custody);
            Ok(())
        }) {
            Ok(job) => self.installation.release.job = Some(job),
            Err(cause) => {
                if let Some(custody) = input {
                    if successful {
                        self.installation.completion = custody._completion;
                    }
                    self.installation.inherited = custody._inherited;
                }
                self.installation.release.admission = Some(cause);
                return Err(AuthorityIssue::Adapter);
            }
        }
        Ok(false)
    }
}

impl Ownership {
    /// Release errors remain available independently of an earlier installation failure.
    pub(in super::super) fn release_failure(&self) -> Option<&(dyn Error + 'static)> {
        self.release.failure_source()
    }
}

#[cfg(test)]
#[path = "release/tests.rs"]
mod tests;
