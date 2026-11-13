// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Create an observed contained family and commit its single resume under retained input ownership.
use super::*;

/// Keep command/environment storage charged until the finite creation attempt returns.
struct ChargedInput {
    input: WindowsLaunchInput,
    _claim: sugarsubstitute_contracts::execution::MemoryClaim,
}
/// Transfer the original admitted input into retained native state before the first effect.
pub(super) fn create(
    shared: SharedOwner,
    input: WindowsLaunchInput,
    context: JobContext,
    ticket: LaunchTicket,
) -> Result<(), RegistryError> {
    let bytes = input.required_reservation_bytes() - crate::registry_input::FAMILY_STORAGE_BYTES;
    let claim =
        context.memory().claim(bytes).map_err(|cause| RegistryError::new("input_memory", cause))?;
    // The accepted creation reservation and its exact claim now fund command/environment
    // storage before releasing the original storage job's allocation admission.
    let mut input = input;
    if let Some(launch) = &mut input.preparation {
        launch.transfer_input_memory();
    }
    let mut charged = ChargedInput { input, _claim: claim };
    let family = Family::allocate_owned(context.memory())?;
    let mut unwind = CreationOutputGuard { shared: shared.clone(), armed: true };
    let mut state = lock(&shared.native);
    state.family = Some(family);
    let result = create_owned(&shared, &mut state, &mut charged.input, &context, ticket);
    if let Err(cause) = &result {
        state.failure = Some(cause.clone());
        if let Some(family) = &state.family {
            family.request_output_close();
        }
    }
    unwind.armed = false;
    result
}
/// Bind root observation before committing the single contained resume.
fn create_owned(
    shared: &Shared,
    state: &mut NativeState,
    input: &mut WindowsLaunchInput,
    context: &JobContext,
    ticket: LaunchTicket,
) -> Result<(), RegistryError> {
    let family = state.family.as_mut().ok_or_else(|| RegistryError::invalid("creation_owner"))?;
    family.preparation_image = input.preparation.take();
    family.cache_family = family
        .preparation_image
        .as_ref()
        .and_then(|launch| launch.provisioning())
        .map(|launch| launch.family());
    if stopped(context) || shared.admission.load(Ordering::Acquire) == CLOSED {
        return Err(RegistryError::invalid("creation_canceled_before_effect"));
    }
    family.adopt_fixture_events(input)?;
    family.adopt_lifecycle(input)?;
    family.adopt_purpose(input)?;
    family.adopt_backend_custody(input)?;
    family.adopt_output(input, ticket.binding)?;
    family.prepare_inheritance()?;
    let root = shared.port.create(family, input)?;
    state.creation = Some(CreationFacts { root, resumed: false });
    let duplicate = shared.port.duplicate(family, 0)?;
    let permit = lock(&shared.permit)
        .take()
        .ok_or_else(|| RegistryError::invalid("root_watch_reservation"))?;
    state.watch = bind(permit, duplicate);
    match ticket.binding {
        FamilyBinding::Desktop(run) => shared.port.enroll_desktop(family, run, ticket.process)?,
        FamilyBinding::Diagnostic(key) => {
            shared.port.enroll_purpose(family, PurposeKey::Diagnostic(key), ticket.process)?
        }
        FamilyBinding::BackendCustody(key) => {
            shared.port.enroll_backend_custody(family, key, ticket.process)?
        }
        FamilyBinding::Preparation(_) => {
            // A preparation helper's update-attempt channel enrolls the suspended root.
            if let Some(key) = family.purpose_key {
                shared.port.enroll_purpose(family, key, ticket.process)?
            }
        }
        FamilyBinding::Managed(_) => {}
    }
    family.validate_output_for_resume()?;
    if stopped(context)
        || shared
            .admission
            .compare_exchange(OPEN, RESUME_COMMITTED, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
    {
        shared.admission.store(CLOSED, Ordering::Release);
        return Err(RegistryError::invalid("creation_canceled_contained"));
    }
    // The CAS is the linearization point for already-admitted resume. A later stop
    // closes future work while retaining this finite committed native operation.
    let resumed = shared.port.resume(family);
    state.creation = Some(CreationFacts { root, resumed: family.resumed });
    resumed
}
/// Consume the already reserved observer exactly once for its original native process.
pub(super) fn bind(permit: ProcessWatchPermit, input: WatchInput) -> Option<WatchJob> {
    match input {
        WatchInput::Native(handle) => Some(permit.bind(handle)),
        #[cfg(test)]
        WatchInput::Simulated => Some(permit.cancel_unbound()),
    }
}
/// Check the original job's cancellation and absolute cutoff at effect boundaries.
fn stopped(context: &JobContext) -> bool {
    context.cancellation_requested()
        || context.deadline().is_some_and(|deadline| context.now() >= deadline)
}
/// Close adopted output admission if a native boundary unwinds before returning its result.
struct CreationOutputGuard {
    shared: SharedOwner,
    armed: bool,
}
impl Drop for CreationOutputGuard {
    fn drop(&mut self) {
        if self.armed
            && let Some(family) = &lock(&self.shared.native).family
        {
            family.request_output_close();
        }
    }
}
