// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Admit viewer storage and explicit source loading before retained backing preparation.

use super::*;
use sugarsubstitute_execution::OwnedInputError;

impl Session {
    /// Startup storage shares the original retained admission used by every later resource.
    pub(super) fn submit_initialization(&mut self) -> Result<(), Box<dyn Error>> {
        let runtime = self.runtime.as_ref().ok_or("Canvas execution is unavailable")?;
        let gpu = self.config.gpu.is_some();
        let spec = self.spec(
            Operation::Infrastructure,
            JobClass::InfrastructureLifecycle,
            budget::reservation_bytes(self.config.maximum_extent, gpu),
            Duration::from_secs(30),
            None,
        )?;
        let presentation = budget::presentation_bytes(self.config.maximum_extent, gpu);
        let anchor = self.config.cache.clone();
        let capture = self.config.capture;
        let verify_native = self.config.verify_native;
        match runtime.client.submit_blocking(&runtime.scope, spec, move |context| {
            work::initialize(anchor, capture, verify_native, presentation, &context)
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

    /// The typed owned-input API keeps the exact original intact on admission rejection.
    pub(super) fn submit_load(&mut self) -> Result<(), Box<dyn Error>> {
        let runtime = self.runtime.as_ref().ok_or("Canvas execution is unavailable")?;
        let resources = self.resources.as_ref().ok_or("Canvas storage is unavailable")?.clone();
        let pending = self.pending_open.as_ref().ok_or("Canvas source intent is unavailable")?;
        let intent = pending.intent;
        let expected = pending.expected;
        #[cfg(feature = "native-composition")]
        let acquirer = match &pending.source {
            OpenSource::Image(_) => None,
            OpenSource::Acquired(owner) => Some(owner.clone()),
        };
        #[cfg(feature = "native-composition")]
        let extra = acquirer.as_ref().map_or(0, |owner| owner.reservation_bytes());
        #[cfg(not(feature = "native-composition"))]
        let extra = 0;
        let reservation =
            work::IMPORT_BYTES.checked_add(extra).ok_or("Source reservation overflow")?;
        let spec = self.spec(
            Operation::Infrastructure,
            JobClass::InfrastructureOrdinary,
            reservation,
            Duration::from_secs(300),
            None,
        )?;
        match runtime.client.submit_blocking_owned_input(
            &runtime.scope,
            spec,
            &mut self.pending_open,
            move |open, context| {
                #[cfg(feature = "native-composition")]
                let originals = match open.source {
                    OpenSource::Image(originals) => originals,
                    OpenSource::Acquired(owner) => {
                        owner.acquire(resources.cache.clone(), &resources.memory, &context)?
                    }
                };
                #[cfg(not(feature = "native-composition"))]
                let OpenSource::Image(originals) = open.source;
                work::load(originals, open.intent, open.reuse, &resources, &context)
            },
        ) {
            Ok(job) => {
                self.loading = Some(Loading {
                    intent,
                    expected,
                    job,
                    #[cfg(feature = "native-composition")]
                    acquirer,
                });
            }
            Err(OwnedInputError::Admission(cause)) if activity::retryable(&cause) => {}
            Err(cause) => return Err(cause.into()),
        }
        Ok(())
    }
}
