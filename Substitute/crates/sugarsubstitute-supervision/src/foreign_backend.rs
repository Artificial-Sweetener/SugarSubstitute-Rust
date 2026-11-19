// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain a ComfyUI this application did not start, and close it only as the exact process it
//! was verified to be.
//!
//! The caller first proves, over an established connection, which process answers on the
//! application's backend port and that it answers as ComfyUI. This owner then opens that
//! process by its identifier and creation time and keeps the handle: every later observation
//! and the close act on the retained kernel object, never on an identifier that may have been
//! reused. A ComfyUI started by a copy of this application is never retained here; that copy
//! is resolved as a running application, not as a stray backend.
use crate::{
    ClaimError,
    backend_listener::BackendPeerIdentity,
    native_claims::files::{self, Budget, Handle},
    profile_conflict::processes::{self, Process},
};
use std::{path::PathBuf, sync::Arc};
use sugarsubstitute_contracts::execution::JobContext;

/// How far up a process's ancestry the application looks for one of its own copies.
const ANCESTRY_BOUND: usize = 64;

/// What retaining the listener found.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ForeignBackendOwner {
    /// A ComfyUI nothing of this application started; its image is retained for closing.
    Foreign(PathBuf),
    /// The listener belongs to a running copy of this application and was left alone.
    Application,
}

/// The verified listener, its descendants and the handles that pin them.
#[derive(Default)]
pub struct ForeignBackend {
    memory: Option<Arc<sugarsubstitute_contracts::execution::MemoryClaim>>,
    processes: Vec<Process>,
    handles: Vec<Handle>,
}

impl ForeignBackend {
    /// The memory `retain` claims from its job for the process list and the handles it keeps.
    pub const RETAIN_BYTES: u64 = 1024 * 1024;

    /// Hold nothing until a verified peer is retained.
    pub fn new() -> Self {
        Self::default()
    }

    /// Open the verified `peer` with the right to end it, unless a copy of this application
    /// started it. `application` is the file name of this application's own images, compared
    /// without regard to case.
    ///
    /// # Errors
    /// Rejects a repeated call, expired work, a peer that exited or whose identifier now names
    /// another process, and native failures.
    pub fn retain(
        &mut self,
        peer: BackendPeerIdentity,
        application: &str,
        context: &JobContext,
    ) -> Result<ForeignBackendOwner, ClaimError> {
        check(context)?;
        if self.memory.is_some() {
            return Err(ClaimError::invalid("foreign_backend_repeated_retain"));
        }
        let memory = Arc::new(
            context
                .memory()
                .claim(Self::RETAIN_BYTES)
                .map_err(|cause| ClaimError::new("foreign_backend_memory", cause))?,
        );
        let budget = Budget::Protected(memory.clone());
        self.memory = Some(memory);
        let listed = processes::listed(budget.clone(), &mut self.handles)?;
        let rows: Vec<(u32, u32, &str)> =
            listed.iter().map(|row| (row.pid, row.parent, row.name.as_str())).collect();
        if started_by(&rows, peer.process_id(), application) {
            return Ok(ForeignBackendOwner::Application);
        }
        let index = Process::open(peer.process_id(), true, budget.clone(), &mut self.processes)?;
        if self.processes[index].birth != peer.creation_time_100ns() {
            return Err(ClaimError::invalid("foreign_backend_identity_changed"));
        }
        if self.processes[index].exited()? {
            return Err(ClaimError::invalid("foreign_backend_exited"));
        }
        check(context)?;
        processes::family(index, true, &mut self.processes, budget, &mut self.handles)?;
        Ok(ForeignBackendOwner::Foreign(self.processes[index].image.clone()))
    }

    /// End every retained process, descendants before the listener itself.
    ///
    /// # Errors
    /// Rejects expired work, a call before anything is retained, and native failures; processes
    /// already ended stay ended.
    pub fn close(&mut self, context: &JobContext) -> Result<(), ClaimError> {
        check(context)?;
        if self.processes.is_empty() {
            return Err(ClaimError::invalid("foreign_backend_not_retained"));
        }
        for process in self.processes.iter().rev() {
            process.force()?;
        }
        Ok(())
    }

    /// Whether every retained process has exited.
    ///
    /// # Errors
    /// Preserves the native wait failure.
    pub fn exited(&self) -> Result<bool, ClaimError> {
        for process in &self.processes {
            if !process.exited()? {
                return Ok(false);
            }
        }
        Ok(true)
    }

    /// Release every handle; the processes themselves are left as they are.
    ///
    /// # Errors
    /// Returns the first handle that could not be closed, retaining it for another attempt.
    pub fn retire(&mut self) -> Result<(), ClaimError> {
        for process in &mut self.processes {
            process.handle.close()?;
        }
        self.processes.clear();
        files::close_all(&mut self.handles)?;
        self.memory = None;
        Ok(())
    }
}

/// A finite operation checks cancellation before every externally visible action.
fn check(context: &JobContext) -> Result<(), ClaimError> {
    if context.cancellation_requested()
        || context.deadline().is_none_or(|until| context.now() >= until)
    {
        Err(ClaimError::invalid("foreign_backend_operation_expired"))
    } else {
        Ok(())
    }
}

/// Whether `pid`, or any process above it in `rows` (identifier, parent, image file name), runs
/// an image named `application`. A parent that has exited ends the walk: its identifier may
/// since name an unrelated process.
fn started_by(rows: &[(u32, u32, &str)], pid: u32, application: &str) -> bool {
    let mut current = pid;
    for _ in 0..ANCESTRY_BOUND {
        let Some(&(_, parent, name)) = rows.iter().find(|row| row.0 == current) else {
            return false;
        };
        if name.eq_ignore_ascii_case(application) {
            return true;
        }
        if parent == 0 || parent == current {
            return false;
        }
        current = parent;
    }
    false
}

#[cfg(test)]
mod tests {
    use super::started_by;

    const ROWS: [(u32, u32, &str); 6] = [
        (4, 0, "System"),
        (100, 4, "explorer.exe"),
        (200, 100, "SugarSubstitute.exe"),
        (210, 200, "python.exe"),
        (300, 100, "cmd.exe"),
        (310, 300, "python.exe"),
    ];

    #[test]
    fn a_backend_under_a_copy_of_the_application_is_the_applications_own() {
        assert!(started_by(&ROWS, 210, "sugarsubstitute.exe"));
        assert!(started_by(&ROWS, 200, "sugarsubstitute.exe"));
    }

    #[test]
    fn a_backend_started_from_a_shell_is_foreign() {
        assert!(!started_by(&ROWS, 310, "sugarsubstitute.exe"));
        assert!(!started_by(&ROWS, 100, "sugarsubstitute.exe"));
    }

    #[test]
    fn an_unlisted_process_or_a_missing_parent_ends_the_walk_as_foreign() {
        assert!(!started_by(&ROWS, 999, "sugarsubstitute.exe"));
        let orphan = [(500, 499, "python.exe")];
        assert!(!started_by(&orphan, 500, "sugarsubstitute.exe"));
    }

    #[test]
    fn a_process_list_that_loops_ends_the_walk() {
        let looped = [(1, 2, "a.exe"), (2, 1, "b.exe")];
        assert!(!started_by(&looped, 1, "sugarsubstitute.exe"));
    }
}
