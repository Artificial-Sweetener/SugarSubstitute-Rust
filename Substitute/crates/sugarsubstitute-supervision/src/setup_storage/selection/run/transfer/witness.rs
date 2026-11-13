// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain original trial ownership independently of its coordinator for settled finalization.
use super::*;
use std::os::windows::io::OwnedHandle;

/// Independent storage-worker reservation for one selection owner and one protected operation.
/// The witness does not hash application payloads or borrow the native custody pool's slots.
pub const TRIAL_WITNESS_RESERVATION_BYTES: u64 = 2 * 1024 * 1024;

/// Sender-owned live exclusion and snapshot for a separately supervised cleanup witness.
/// Creating or dropping a transfer does not publish selection or prove helper enrollment.
pub struct TrialWitnessTransfer(custody::Transfer);
impl TrialWitnessTransfer {
    /// Include only these handles, ordered run then snapshot, in the explicit transfer.
    /// Keep this owner until the receiver acknowledges authenticated trial custody.
    pub fn handles(&self) -> [BorrowedHandle<'_>; 2] {
        self.0.handles()
    }
}

impl TrialApplication {
    /// Share original run custody before health publication or descendant admission.
    /// The coordinator retains its own readiness authority. This transfer does not grant
    /// the receiver permission to start an application or declare startup health.
    /// # Errors
    /// Rejects changed selection, cancellation and native snapshot/duplication failures.
    pub fn prepare_witness(
        &self,
        context: &JobContext,
    ) -> Result<TrialWitnessTransfer, SelectionFailure> {
        custody::Transfer::new(&self.trial, context).map(TrialWitnessTransfer)
    }
}

/// Cleanup-only ownership of the exact trial, continuously retaining native run exclusion in
/// this process's own adopted slot, so it outlives the coordinator that handed it over.
/// The witness does not verify executable payloads or offer launch/health acceptance operations.
/// Retain through actual family settlement and authenticated coordinator closing or exit.
pub struct TrialWitness {
    run: InstallationRun,
    trial: OwnedTrial,
    _generation: Directory,
}
impl TrialWitness {
    /// Receive already-owned outputs from the authenticated helper's native duplication channel.
    /// These handles need not be inheritable. The channel must authenticate the sender and own
    /// each actual destination output before calling; numeric command fields are insufficient.
    /// This consumes the run then snapshot handles even on validation failure, and performs the
    /// same installation, snapshot and generation checks as initial-entry receipt.
    /// Reserve `TRIAL_WITNESS_RESERVATION_BYTES` on an independently admitted storage worker,
    /// including opening its `SelectionStore`. The native process-custody pool is separate.
    /// # Errors
    /// Rejects foreign or nonexclusive objects, stale records, incorrect generation identity,
    /// cancellation and native storage failures. It neither reopens nor reconstructs run custody.
    pub fn receive_transferred(
        store: Arc<SelectionStore>,
        handles: [OwnedHandle; 2],
        running_generation: &Path,
        context: &JobContext,
    ) -> Result<Self, SelectionFailure> {
        let [run, snapshot] = handles;
        Self::receive(
            store,
            InheritedTrial { run: File::from(run), snapshot: File::from(snapshot), adopted: None },
            running_generation,
            context,
        )
    }

    /// Bind captured live custody to this installation and the receiver's actual generation.
    /// Must finish before the coordinator publishes health or admits descendant work.
    /// The caller authenticates the receiver identity; directory spelling alone is insufficient.
    /// # Errors
    /// Rejects changed selection, foreign handles, wrong generation, links, invalid protection,
    /// cancellation and native failures. Never publishes or acquires replacement run custody.
    pub fn receive(
        store: Arc<SelectionStore>,
        inherited: InheritedTrial,
        running_generation: &Path,
        context: &JobContext,
    ) -> Result<Self, SelectionFailure> {
        let (run, trial) = custody::receive(store, inherited, context)?;
        let store = &run.custody.store;
        let path =
            store._installation.path().join("releases").join(trial.attempt.generation.id.as_str());
        let generation = crate::durable_storage::protection::with(context, |protection| {
            protection.directory(
                &path,
                crate::durable_storage::protected::directories::Mode::Existing,
                context,
            )?;
            Directory::open(&path, false)
        })?;
        let running = Directory::open(running_generation, false)?;
        if running.identity()? != generation.identity()? {
            return Err(SelectionFailure::RunOwnership);
        }
        check(context)?;
        Ok(Self { run, trial, _generation: generation })
    }

    /// Exact attempt for authenticated helper enrollment and teardown correlation.
    pub fn attempt(&self) -> &Trial {
        self.trial.attempt()
    }

    /// Finalize after all owned families settle and the coordinator either exits or sends its
    /// authenticated closing request. The coordinator retains its own run exclusion until exit.
    /// `verdict` classifies how the coordinator ended (`HostExit::verdict`). Preserve committed
    /// acceptance and any coordinator settlement despite a lost coordinator result; settle an
    /// unresolved trial once. Retain this owner on failure and until the finalization result has
    /// been observed.
    /// Run on an independently admitted storage worker under its original cleanup deadline;
    /// a cancelled or expired native-custody worker cannot authorize this publication.
    /// # Errors
    /// Preserves changed activation history, cancellation and native publication failures.
    pub fn finish_settled(
        &self,
        verdict: sugarsubstitute_contracts::installation::TrialVerdict,
        context: &JobContext,
    ) -> Result<InstallationRecord, SelectionFailure> {
        self.run.finish_trial(&self.trial, verdict, context)
    }
}
