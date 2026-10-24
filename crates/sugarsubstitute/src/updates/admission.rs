// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Preserve updater phases while the execution owner retires earlier memory reservations.
use crate::authority_clock::Receipt;
use std::io;
use sugarsubstitute_contracts::execution::{AdmissionError, ExecutionPort, JobId, Moment, Scope};
use sugarsubstitute_supervision::update_preparation::Worker;

/// Submit once without blocking the event loop. `None` means memory admission deferred the
/// unchanged phase and armed the executor's capacity wake. The caller retains its original
/// deadline and retries on activity; this does not renew time or declare work completed.
/// All other rejection causes remain errors. A UTC receipt belongs only to an admitted job.
pub(crate) fn submit(
    worker: &mut Worker,
    execution: &impl ExecutionPort,
    scope: &Scope,
    id: JobId,
    until: Moment,
) -> io::Result<Option<Receipt>> {
    let receipt = Receipt::default();
    let sample = receipt.clone();
    match worker.submit(execution, scope, id, until, move || sample.now()) {
        Ok(()) => Ok(Some(receipt)),
        Err(cause)
            if matches!(
                cause.get_ref().and_then(|cause| cause.downcast_ref::<AdmissionError>()),
                Some(AdmissionError::MemorySaturated { .. })
            ) =>
        {
            Ok(None)
        }
        Err(cause) => Err(cause),
    }
}
