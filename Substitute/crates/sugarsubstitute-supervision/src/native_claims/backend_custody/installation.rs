// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exchange bounded trial-storage requests with the helper's independently owned storage jobs.
mod worker;
use crate::{ClaimError, native_claims::files::Budget, setup_storage::TrialWitness};
use std::{
    os::windows::io::OwnedHandle,
    path::PathBuf,
    sync::{Arc, Mutex},
};
use sugarsubstitute_contracts::{
    execution::{CompletionWake, Moment},
    installation::HostExit,
};
use sugarsubstitute_execution::backend_custody::BackendCustodyWake;
pub use worker::BackendCustodyInstallation;

#[derive(Clone, Copy, Eq, PartialEq)]
enum Phase {
    Dormant,
    AdmissionRequested,
    Admitting,
    Ready,
    SettlementRequested,
    Settling,
    Settled,
    Failed,
}

struct Data {
    phase: Phase,
    claimed: bool,
    until: Option<Moment>,
    cleanup_until: Option<Moment>,
    input: Option<[OwnedHandle; 2]>,
    exit: Option<HostExit>,
    witness: Option<Arc<TrialWitness>>,
    native_wake: Option<Arc<dyn CompletionWake>>,
    failure: Option<ClaimError>,
}
struct Shared {
    data: Mutex<Data>,
    generation: PathBuf,
    wake: Arc<dyn CompletionWake>,
    _memory: Budget,
}

/// Wake the original native observation worker after storage completion has been consumed.
pub(super) struct NativeWake(pub(super) BackendCustodyWake);
impl CompletionWake for NativeWake {
    fn wake(&self) -> Result<(), sugarsubstitute_contracts::execution::ExecutionError> {
        self.0.observed();
        Ok(())
    }
}

/// Native requests retain their exact input and accepted witness outside worker outcomes.
#[derive(Clone)]
pub(super) struct Control(Arc<Shared>);
impl Control {
    pub(super) fn new(memory: Budget, generation: PathBuf, wake: Arc<dyn CompletionWake>) -> Self {
        Self(Arc::new(Shared {
            data: Mutex::new(Data {
                phase: Phase::Dormant,
                claimed: false,
                until: None,
                cleanup_until: None,
                input: None,
                exit: None,
                witness: None,
                native_wake: None,
                failure: None,
            }),
            generation,
            wake,
            _memory: memory,
        }))
    }

    /// Include the fixed controller, one worker observer and bounded invocation path in preflight.
    pub(super) fn storage_bytes(generation: &std::path::Path) -> usize {
        size_of::<Shared>()
            + 2 * size_of::<usize>()
            + size_of::<BackendCustodyInstallation>()
            + size_of::<NativeWake>()
            + 2 * size_of::<usize>()
            + generation.as_os_str().len() * 2
    }

    /// Only the helper's lifetime owner may take the single job-observation controller.
    pub(super) fn worker(&self) -> Result<BackendCustodyInstallation, ClaimError> {
        let mut data = crate::registry::lock(&self.0.data);
        if data.claimed {
            return Err(ClaimError::invalid("installation_witness_worker_repeated"));
        }
        data.claimed = true;
        Ok(BackendCustodyInstallation::new(self.clone()))
    }

    /// Adopt both authenticated destination outputs before waking the storage owner.
    pub(super) fn admit(
        &self,
        input: [OwnedHandle; 2],
        until: Moment,
        wake: Arc<dyn CompletionWake>,
    ) -> Result<(), ClaimError> {
        {
            let mut data = crate::registry::lock(&self.0.data);
            if data.phase != Phase::Dormant || !data.claimed {
                return Err(ClaimError::invalid("installation_witness_admission_phase"));
            }
            data.input = Some(input);
            data.until = Some(until);
            data.native_wake = Some(wake);
            data.phase = Phase::AdmissionRequested;
        }
        self.wake_owner()
    }

    /// Read only observed admission; successful storage work alone cannot synthesize readiness.
    pub(super) fn ready(&self) -> Result<bool, ClaimError> {
        let data = crate::registry::lock(&self.0.data);
        if let Some(cause) = &data.failure {
            return Err(cause.clone());
        }
        Ok(data.phase == Phase::Ready)
    }

    /// Called only after native family settlement. `exit` is how the coordinator ended, its
    /// authenticated finish or its observed process exit; `None` means neither happened yet.
    /// Preserve the first cleanup cutoff while admission completion is still being consumed.
    pub(super) fn settle(&self, until: Moment, exit: Option<HostExit>) -> Result<bool, ClaimError> {
        let authorized = exit.is_some();
        let requested = {
            let mut data = crate::registry::lock(&self.0.data);
            if let Some(cause) = &data.failure {
                return Err(cause.clone());
            }
            match data.phase {
                Phase::Dormant | Phase::Settled => return Ok(true),
                Phase::Ready => {
                    if !authorized {
                        return Ok(false);
                    }
                    let cutoff = *data.cleanup_until.get_or_insert(until);
                    data.until = Some(cutoff);
                    data.exit = exit;
                    data.phase = Phase::SettlementRequested;
                    true
                }
                Phase::AdmissionRequested | Phase::Admitting => {
                    if authorized {
                        data.cleanup_until.get_or_insert(until);
                    }
                    false
                }
                Phase::SettlementRequested | Phase::Settling => false,
                Phase::Failed => return Err(ClaimError::invalid("installation_witness_failed")),
            }
        };
        if requested {
            self.wake_owner()?;
        }
        Ok(false)
    }

    fn wake_owner(&self) -> Result<(), ClaimError> {
        self.0
            .wake
            .wake()
            .map_err(|cause| ClaimError::new("installation_witness_owner_wake", cause))
    }

    fn fail(&self, cause: ClaimError) -> Result<(), ClaimError> {
        let wake = {
            let mut data = crate::registry::lock(&self.0.data);
            if data.failure.is_none() {
                data.failure = Some(cause.clone());
            }
            data.phase = Phase::Failed;
            data.native_wake.clone()
        };
        let notified = wake
            .map(|wake| wake.wake())
            .transpose()
            .map(|_| ())
            .map_err(|cause| ClaimError::new("installation_witness_native_wake", cause));
        ClaimError::finish(Err(cause), notified)
    }
}
