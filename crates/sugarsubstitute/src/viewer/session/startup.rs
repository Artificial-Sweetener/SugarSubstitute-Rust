// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Admit viewer storage and explicit source loading before retained backing preparation.

use super::*;
use sugarsubstitute_execution::OwnedInputError;

impl Session {
    /// Startup storage shares the original retained admission used by every later resource.
    pub(super) fn submit_initialization(&mut self) -> Result<(), Box<dyn Error>> {
        let runtime = self.runtime.as_ref().ok_or("Canvas execution is unavailable")?;
        let spec = self.spec(
            Operation::Infrastructure,
            JobClass::InfrastructureLifecycle,
            if self.options.gpu.is_some() {
                work::GPU_RETAINED_BYTES
            } else {
                work::RETAINED_BYTES
            },
            Duration::from_secs(30),
            None,
        )?;
        let anchor = self.options.cache.clone();
        let capture = self.options.capture;
        let verify_native = self.options.verify_native;
        match runtime.client.submit_blocking(&runtime.scope, spec, move |context| {
            work::initialize(anchor, capture, verify_native, &context)
        }) {
            Ok(job) => {
                self.initializing = Some(job);
                self.initialization_pending = false;
            }
            Err(cause) if activity::retryable(&cause) => {}
            Err(cause) => return Err(cause.into()),
        }
        Ok(())
    }

    /// The typed owned-input API keeps the exact selected path intact on admission rejection.
    pub(super) fn submit_load(&mut self) -> Result<(), Box<dyn Error>> {
        let runtime = self.runtime.as_ref().ok_or("Canvas execution is unavailable")?;
        let resources = self.resources.as_ref().ok_or("Canvas storage is unavailable")?.clone();
        let pending = self.pending_open.as_ref().ok_or("Canvas source intent is unavailable")?;
        let intent = pending.intent;
        let expected = pending.expected;
        let spec = self.spec(
            Operation::Infrastructure,
            JobClass::InfrastructureOrdinary,
            work::IMPORT_BYTES,
            Duration::from_secs(300),
            None,
        )?;
        match runtime.client.submit_blocking_owned_input(
            &runtime.scope,
            spec,
            &mut self.pending_open,
            move |open, context| {
                work::load(open.path, open.intent, open.reuse, &resources, &context)
            },
        ) {
            Ok(job) => self.loading = Some(Loading { intent, expected, job }),
            Err(OwnedInputError::Admission(cause)) if activity::retryable(&cause) => {}
            Err(cause) => return Err(cause.into()),
        }
        Ok(())
    }
}
