// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Transfer only complete Domain requests paired with unchanged original custody.

use super::{
    CapturedEntryEnvironment, EntryEnvironmentFailure, EntryPreparationError,
    EntryPreparationInputError, EntryPreparationSuccess, owner::EntryPreparationOwner,
};
use crate::ExecutionClient;
use std::{fmt, path::PathBuf};
use sugarsubstitute_contracts::{
    configuration::StartupConfiguration,
    coordinator_entry::{EntryLaunchRequest, EntrySetupBudget},
};

/// Borrow an inert Domain request while retaining any actual preparation origin.
/// Native authority is absent. Data and final original custody cannot separate.
pub struct EntryRequestInput {
    request: Box<EntryLaunchRequest>,
    preparation: Option<EntryPreparationOwner>,
    #[cfg(any(windows, test))]
    transport_memory: Option<sugarsubstitute_contracts::execution::MemoryClaim>,
}
impl From<EntryLaunchRequest> for EntryRequestInput {
    fn from(request: EntryLaunchRequest) -> Self {
        Self {
            request: Box::new(request),
            preparation: None,
            #[cfg(any(windows, test))]
            transport_memory: None,
        }
    }
}
impl EntryRequestInput {
    /// Keep the transport's actual storage charge across a checked source handback.
    #[cfg(any(windows, test))]
    pub(crate) fn retain_transport_memory(
        &mut self,
        claim: sugarsubstitute_contracts::execution::MemoryClaim,
    ) {
        self.transport_memory = Some(claim);
    }
    /// Borrow the authoritative Domain request, preserving its exact grammar.
    pub fn request(&self) -> &EntryLaunchRequest {
        &self.request
    }
    /// Borrow exact original UTF16 environment units without an owned projection.
    pub fn units(&self) -> &[u16] {
        self.request.environment()
    }
    pub(super) fn adopt_owner(
        mut self,
        owner: EntryPreparationOwner,
    ) -> Result<Self, (EntryPreparationInputError, Self, EntryPreparationOwner)> {
        if self.preparation.as_ref().is_some_and(|existing| !existing.same_owner(&owner)) {
            return Err((EntryPreparationInputError::ForeignOwner, self, owner));
        }
        let replaced = self.preparation.replace(owner);
        drop(replaced);
        Ok(self)
    }
    pub(crate) fn original_owner(&self) -> Option<&EntryPreparationOwner> {
        self.preparation.as_ref()
    }
    #[cfg(any(windows, test))]
    pub(crate) fn check_original_core(
        &self,
        core: &crate::admission::Core,
    ) -> Result<Option<sugarsubstitute_contracts::execution::Moment>, EntryPreparationError> {
        let Some(owner) = self.original_owner() else {
            return Ok(None);
        };
        owner
            .checked(|| {
                let binding = owner.original_binding()?;
                binding.check_publication(core, binding.setup())?;
                Ok(binding.setup().until())
            })
            .map(Some)
            .map_err(|cause| EntryPreparationError::new(cause, owner.clone()))
    }

    #[cfg(any(windows, test))]
    pub(crate) fn observe_current_original_core(
        &self,
        core: &crate::admission::Core,
    ) -> Result<(), EntryPreparationError> {
        let Some(owner) = self.original_owner() else {
            return Ok(());
        };
        owner
            .checked(|| {
                let binding = owner.original_binding()?;
                binding.check(core, binding.setup())
            })
            .map(|_| ())
            .map_err(|cause| EntryPreparationError::new(cause, owner.clone()))
    }

    /// Observe genuine original controls after a completed external step.
    /// This remains observable after publication closes and never renews setup.
    /// # Errors
    /// Retains the actual original cause and unchanged whole owner on rejection.
    pub fn observe_original(
        &self,
        client: &ExecutionClient,
        original: EntrySetupBudget,
    ) -> Result<(), EntryPreparationError> {
        self.observe_original_core(&client.core, original)
    }
    pub(crate) fn observe_original_core(
        &self,
        core: &crate::admission::Core,
        original: EntrySetupBudget,
    ) -> Result<(), EntryPreparationError> {
        let Some(owner) = self.original_owner() else {
            return Ok(());
        };
        owner
            .checked(|| owner.original_binding()?.check(core, original))
            .map(|_| ())
            .map_err(|cause| EntryPreparationError::new(cause, owner.clone()))
    }

    /// Validate captured origin against the receiving Core and unchanged setup.
    /// Inert Domain inputs have no preparation origin and keep Domain validation.
    /// # Errors
    /// Foreign Core/setup or revoked original publication retains its genuine
    /// source and whole owner; validation does not renew any admission deadline.
    pub fn validate_original(
        &self,
        client: &ExecutionClient,
        original: EntrySetupBudget,
    ) -> Result<(), EntryPreparationError> {
        let Some(owner) = self.original_owner() else {
            return Ok(());
        };
        owner
            .checked(|| owner.original_binding()?.check_publication(&client.core, original))
            .map(|_| ())
            .map_err(|cause| EntryPreparationError::new(cause, owner.clone()))
    }
}
impl fmt::Debug for EntryRequestInput {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("EntryRequestInput { retained, redacted }")
    }
}
impl CapturedEntryEnvironment {
    /// Consume copied units into the sole Domain validator with unchanged custody.
    /// # Errors
    /// Original control or Domain validation failures retain their genuine source
    /// and original owner. Supplied paths/configuration keep Domain grammar.
    #[allow(
        clippy::too_many_arguments,
        reason = "Preserve the authoritative Domain launch inputs at their actual owned transfer"
    )]
    pub fn into_request(
        self,
        configuration: StartupConfiguration,
        package: PathBuf,
        metadata: PathBuf,
        cube: PathBuf,
        script: PathBuf,
        profile_directory: PathBuf,
        originals: Option<sugarsubstitute_contracts::coordinator_entry::EntryOriginalsReference>,
    ) -> Result<EntryRequestInput, EntryEnvironmentFailure> {
        let Self { units, owner } = self;
        if let Err(cause) = owner.check_publication() {
            return Err(EntryEnvironmentFailure::from_control(cause, owner));
        }
        let request = EntryLaunchRequest::new(
            configuration,
            package,
            metadata,
            cube,
            script,
            profile_directory,
            units,
        )
        .map(|request| match originals {
            Some(reference) => request.with_originals(reference),
            None => request,
        });
        match request {
            Ok(request) => Ok(EntryRequestInput {
                request: Box::new(request),
                preparation: Some(owner),
                #[cfg(any(windows, test))]
                transport_memory: None,
            }),
            Err(source) => Err(EntryEnvironmentFailure::from_shape(source, owner)),
        }
    }
}

/// Preserve the complete prepared wrapper and actual failure on checked rejection.
pub struct EntryRequestAdoptionRejection {
    cause: EntryPreparationError,
    prepared: Box<EntryPreparationSuccess<EntryRequestInput>>,
}
impl EntryRequestAdoptionRejection {
    /// Borrow the genuine retained validation cause.
    pub const fn cause(&self) -> &EntryPreparationError {
        &self.cause
    }
    /// Borrow all original data and custody without splitting the aggregate.
    pub const fn prepared(&self) -> &EntryPreparationSuccess<EntryRequestInput> {
        &self.prepared
    }
}
impl fmt::Debug for EntryRequestAdoptionRejection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("EntryRequestAdoptionRejection { retained, redacted }")
    }
}
impl ExecutionClient {
    /// Adopt actual prepared data for this Core and its unchanged original setup.
    /// # Errors
    /// Foreign Core/setup, current cancellation/expiry or closed publication keeps
    /// the complete original wrapper and genuine failure. Providers run outside locks.
    pub fn adopt_prepared_entry_request(
        &self,
        prepared: EntryPreparationSuccess<EntryRequestInput>,
        original: EntrySetupBudget,
    ) -> Result<EntryRequestInput, EntryRequestAdoptionRejection> {
        let checked = prepared.owner().checked(|| {
            prepared
                .owner()
                .original_binding()
                .and_then(|binding| binding.check_publication(&self.core, original))
        });
        if let Err(cause) = checked {
            let cause = EntryPreparationError::new(cause, prepared.owner().clone());
            return Err(EntryRequestAdoptionRejection { cause, prepared: Box::new(prepared) });
        }
        prepared.into_entry_request().map_err(|(cause, prepared)| {
            let cause = EntryPreparationError::new(cause, prepared.owner().clone());
            EntryRequestAdoptionRejection { cause, prepared: Box::new(prepared) }
        })
    }
}

#[cfg(windows)]
impl ExecutionClient {
    /// Convert the unchanged original setup while retaining actual pre/post controls.
    /// # Errors
    /// Keeps returned counter failure primary and original post-control separate,
    /// with the complete preparation owner retained through every escaped clone.
    pub fn encode_prepared_entry_deadline(
        &self,
        input: &EntryRequestInput,
        original: EntrySetupBudget,
    ) -> Result<sugarsubstitute_contracts::execution::CounterDeadline, crate::EntryTransportError>
    {
        input.validate_original(self, original).map_err(crate::EntryTransportError::Preparation)?;
        let converted = self
            .encode_counter_deadline(original.until())
            .map_err(crate::EntryTransportError::Clock);
        let post =
            input.observe_original(self, original).map_err(crate::EntryTransportError::Preparation);
        match (converted, post) {
            (Err(first), after) => Err(crate::EntryTransportError::prepared(
                first,
                after.err(),
                input.original_owner(),
            )),
            (Ok(_), Err(after)) => {
                Err(crate::EntryTransportError::prepared(after, None, input.original_owner()))
            }
            (Ok(source), Ok(())) => Ok(source),
        }
    }
}
