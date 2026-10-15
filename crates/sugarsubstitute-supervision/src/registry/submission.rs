// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Reserve observation before finite creation and retain state through cancellation.
use super::*;
use sugarsubstitute_contracts::execution::{JobClass, Operation, ScopeSpec};
use sugarsubstitute_execution::windows::NativeCorrelation;

impl WindowsProcessRegistry {
    /// Reserve root observation, then admit finite creation before any native effect.
    /// The registry creates/retains an independent native scope from the supplied
    /// no-deadline specification, with no parent. Job IDs increase in
    /// actual admission order, so `root_watch.id` must precede `creation.id`.
    /// Rejected creation retains only cancellation/retirement of its admitted watch;
    /// its private record automatically retires after actual typed cancellation.
    /// Inputs are moved once and charged by the real creation reservation.
    #[allow(clippy::too_many_arguments)]
    pub fn submit_launch(
        &mut self,
        execution: &ExecutionClient,
        creation_scope: &Scope,
        native_scope_spec: ScopeSpec,
        creation: JobSpec,
        root_watch: JobSpec,
        binding: FamilyBinding,
        input: WindowsLaunchInput,
    ) -> Result<LaunchTicket, RegistryAdmissionError> {
        let mut input = Some(input);
        self.submit_owned(
            execution,
            creation_scope,
            native_scope_spec,
            creation,
            root_watch,
            binding,
            &mut input,
        )
    }
    /// Admit one original diagnostic family without losing rejected input ownership.
    /// Errors preserve `input:Some` until actual creation admission transfers it;
    /// already reserved root observation remains registry-owned and must be observed.
    ///
    /// # Errors
    /// Rejects missing/foreign pins, role predecessor, quota, invalid jobs or actual capacity.
    #[allow(clippy::too_many_arguments)]
    pub fn submit_diagnostic_launch(
        &mut self,
        execution: &ExecutionClient,
        creation_scope: &Scope,
        native_scope_spec: ScopeSpec,
        creation: JobSpec,
        root_watch: JobSpec,
        binding: sugarsubstitute_contracts::incident::DiagnosticKey,
        input: &mut Option<WindowsLaunchInput>,
    ) -> Result<LaunchTicket, RegistryAdmissionError> {
        self.submit_owned(
            execution,
            creation_scope,
            native_scope_spec,
            creation,
            root_watch,
            FamilyBinding::Diagnostic(binding),
            input,
        )
    }
    #[allow(clippy::too_many_arguments)]
    pub(super) fn submit_owned(
        &mut self,
        execution: &ExecutionClient,
        creation_scope: &Scope,
        native_scope_spec: ScopeSpec,
        creation: JobSpec,
        root_watch: JobSpec,
        binding: FamilyBinding,
        input: &mut Option<WindowsLaunchInput>,
    ) -> Result<LaunchTicket, RegistryAdmissionError> {
        let original = input.as_ref().ok_or(RegistryAdmissionError::InvalidSpecification)?;
        if let Some(profile) = &original.profile_custody
            && (binding!=FamilyBinding::Desktop(profile.value.key.run()) || !profile.value.identity.same_as(&self.identity)
                || original.managed_custody.is_some() || original.backend_custody.is_some()
                || original.diagnostic.is_some() || profile.value.published_ack.as_ref().is_none_or(|ack|
                    ack.receipt()!=sugarsubstitute_contracts::backend_custody::BackendCustodyReceipt::ProfilePublished(profile.value.key))) {
                return Err(RegistryAdmissionError::InvalidSpecification);
        }
        if let Some(source) = &original.managed_custody
            && binding != FamilyBinding::Managed(source.resource())
        {
            return Err(RegistryAdmissionError::InvalidSpecification);
        }
        if let FamilyBinding::BackendCustody(key) = binding {
            if original.backend_custody.as_ref().is_none_or(|launch| !launch.validates(key))
                || original.diagnostic.is_some()
                || original.lifecycle.is_some()
                || original.output.is_some()
                || original.fixture_events.is_some()
            {
                return Err(RegistryAdmissionError::InvalidSpecification);
            }
        } else if original.backend_custody.is_some() {
            return Err(RegistryAdmissionError::InvalidSpecification);
        }
        if let FamilyBinding::Diagnostic(key) = binding {
            if original.diagnostic.as_ref().is_none_or(|launch| !launch.validates(key))
                || original.lifecycle.is_some()
                || original.output.is_some()
                || original.fixture_events.is_some()
            {
                return Err(RegistryAdmissionError::InvalidSpecification);
            }
        } else if original.diagnostic.is_some() {
            return Err(RegistryAdmissionError::InvalidSpecification);
        }
        if binding.owner() != self.owner {
            return Err(RegistryAdmissionError::ForeignOwner);
        }
        if original
            .output
            .as_ref()
            .is_some_and(|output| !output.validates(binding) || original.fixture_events.is_some())
        {
            return Err(RegistryAdmissionError::InvalidSpecification);
        }
        if let Some(launch) = &original.lifecycle
            && (binding != FamilyBinding::Desktop(launch.run)
                || original.fixture_events.is_some()
                || launch
                    .enrollment
                    .as_ref()
                    .is_none_or(|enrollment| !launch.bootstrap.matches_enrollment(enrollment)))
        {
            return Err(RegistryAdmissionError::InvalidSpecification);
        }
        if !self.launch_open {
            return Err(RegistryAdmissionError::Closed);
        }
        validate_blocking(creation)?;
        validate_watch(root_watch)?;
        if root_watch.id >= creation.id
            || native_scope_spec.deadline.is_some()
            || creation.reservation_bytes < original.required_reservation_bytes()
        {
            return Err(RegistryAdmissionError::InvalidSpecification);
        }
        let prepared_index = original.profile_custody.as_ref().and_then(|profile| {
            self.records.iter().position(|record| {
                record.as_ref().is_some_and(|record| {
                    record.ticket.process.id.get() == profile.value.key.registry_process()
                        && record.ticket.binding == binding
                        && !record.started
                        && record.creation.is_none()
                        && lock(&record.shared.native).pending_input.is_none()
                })
            })
        });
        if original.profile_custody.is_some() && prepared_index.is_none() {
            return Err(RegistryAdmissionError::InvalidSpecification);
        }
        let kind_count = self
            .records
            .iter()
            .flatten()
            .filter(|record| record.ticket.binding.kind() == binding.kind())
            .count();
        if kind_count - usize::from(prepared_index.is_some()) >= binding.kind().capacity() {
            return Err(RegistryAdmissionError::Saturated);
        }
        // A current run/resource cannot overlap a retained uncertain predecessor.
        if self.records.iter().enumerate().any(|(at, slot)| {
            if prepared_index == Some(at) {
                return false;
            }
            let Some(record) = slot else {
                return false;
            };
            match (record.ticket.binding, binding) {
                (FamilyBinding::Desktop(old), FamilyBinding::Desktop(new)) => {
                    old.instance == new.instance
                }
                (FamilyBinding::Managed(old), FamilyBinding::Managed(new)) => old.id == new.id,
                (FamilyBinding::Diagnostic(old), FamilyBinding::Diagnostic(new)) => {
                    old.owner() == new.owner() && old.role() == new.role()
                }
                (FamilyBinding::BackendCustody(old), FamilyBinding::BackendCustody(new)) => {
                    old.owner() == new.owner()
                }
                _ => false,
            }
        }) {
            return Err(RegistryAdmissionError::Busy);
        }
        let (index, mut record) = if let Some(index) = prepared_index {
            let record = self.records[index].take().ok_or(RegistryAdmissionError::Stale)?;
            if record.root_spec.1 != root_watch.id
                || record.native_scope.spec() != native_scope_spec
            {
                self.records[index] = Some(record);
                return Err(RegistryAdmissionError::InvalidSpecification);
            }
            if record.shared.admission.load(Ordering::Acquire) != OPEN || record.settled {
                self.records[index] = Some(record);
                return Err(RegistryAdmissionError::Closed);
            }
            lock(&record.shared.native).pending_input = input.take();
            (index, record)
        } else {
            let index = self
                .records
                .iter()
                .position(Option::is_none)
                .ok_or(RegistryAdmissionError::Saturated)?;
            let reserved = original
                .profile_custody
                .as_ref()
                .map(|profile| profile.value.key.registry_process());
            let process_id = ProcessId::new(reserved.unwrap_or(self.next_process))
                .map_err(|_| RegistryAdmissionError::IdentityExhausted)?;
            let next = if reserved.is_some() {
                self.next_process
            } else {
                self.next_process.checked_add(1).ok_or(RegistryAdmissionError::IdentityExhausted)?
            };
            let correlation = NativeCorrelation::new(process_id.get())
                .map_err(|_| RegistryAdmissionError::InvalidSpecification)?;
            let native_scope = execution
                .create_native_scope(native_scope_spec, None)
                .map_err(RegistryAdmissionError::Execution)?;
            let root_spec = (native_scope.id(), root_watch.id);
            let permit = execution
                .reserve_process_watch(&native_scope, root_watch, correlation)
                .map_err(RegistryAdmissionError::Execution)?;
            let shared = SharedOwner::new(
                Shared {
                    admission: AtomicU8::new(OPEN),
                    native: Mutex::new(NativeState {
                        establishment: None,
                        custody_destination: None,
                        custody_release: None,
                        pending_input: input.take(),
                        family: None,
                        watch: None,
                        creation: None,
                        cleanup: None,
                        failure: None,
                        forced: false,
                        port: self.port.clone(),
                    }),
                    port: self.port.clone(),
                    permit: Mutex::new(Some(permit)),
                },
                self.identity.clone(),
            );
            let ticket =
                LaunchTicket { process: ProcessKey { owner: self.owner, id: process_id }, binding };
            let record = Record::pending(ticket, shared, native_scope, root_spec);
            self.next_process = next;
            (index, record)
        };
        let shared = record.shared.clone();
        let ticket = record.ticket;
        let custodian = lock(&shared.native).pending_input.as_ref().and_then(|input| {
            input.managed_custody.as_ref().map(|source| source.parent().clone()).or_else(|| {
                input.profile_custody.as_ref().map(|profile| profile.value.parent.clone())
            })
        });
        let job = if let Some(parent) = custodian {
            let mut retained = Some(shared.clone());
            let client = execution.clone();
            execution
                .submit_custodied_process(
                    creation_scope,
                    creation,
                    parent.storage(),
                    &mut retained,
                    move |shared, context, memory| {
                        let input = lock(&shared.native).pending_input.take().ok_or_else(|| {
                            NativeObservationError::new(
                                "custodied_input_owner",
                                RegistryError::invalid("creation_input_owner"),
                            )
                        })?;
                        custodied_creation::Driver::construct(
                            shared, input, context, memory, client, ticket,
                        )
                    },
                )
                .map(CreationJob::Custodied)
        } else {
            execution
                .submit_blocking(creation_scope, creation, move |context| {
                    let input = lock(&shared.native)
                        .pending_input
                        .take()
                        .ok_or_else(|| RegistryError::invalid("creation_input_owner"))?;
                    create(shared, input, context, ticket)
                })
                .map(CreationJob::Blocking)
        };
        match job {
            Ok(job) => record.creation = Some(job),
            Err(cause) => {
                *input = lock(&record.shared.native).pending_input.take();
                record.rejected_launch = true;
                record.closed_native = true;
                record.shared.admission.store(CLOSED, Ordering::Release);
                if let Some(permit) = lock(&record.shared.permit).take() {
                    record.root_watch = Some(permit.cancel_unbound());
                }
                self.records[index] = Some(record);
                return Err(RegistryAdmissionError::Execution(cause));
            }
        }
        self.records[index] = Some(record);
        Ok(ticket)
    }
}
pub(crate) fn validate_blocking(spec: JobSpec) -> Result<(), RegistryAdmissionError> {
    if spec.operation != Operation::NativeLifecycle
        || spec.class != JobClass::InfrastructureLifecycle
        || spec.replacement.is_some()
    {
        return Err(RegistryAdmissionError::InvalidSpecification);
    }
    Ok(())
}
pub(super) fn validate_watch(spec: JobSpec) -> Result<(), RegistryAdmissionError> {
    if spec.operation != Operation::NativeLifecycle
        || spec.class != JobClass::NativeObservation
        || spec.replacement.is_some()
        || spec.deadline.is_some()
        || spec.reservation_bytes < 4096
    {
        return Err(RegistryAdmissionError::InvalidSpecification);
    }
    Ok(())
}
struct ChargedInput {
    input: WindowsLaunchInput,
    _claim: sugarsubstitute_contracts::execution::MemoryClaim,
}
fn create(
    shared: SharedOwner,
    input: WindowsLaunchInput,
    context: JobContext,
    ticket: LaunchTicket,
) -> Result<(), RegistryError> {
    let bytes = input.required_reservation_bytes() - crate::registry_input::FAMILY_STORAGE_BYTES;
    let claim =
        context.memory().claim(bytes).map_err(|cause| RegistryError::new("input_memory", cause))?;
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
fn create_owned(
    shared: &Shared,
    state: &mut NativeState,
    input: &mut WindowsLaunchInput,
    context: &JobContext,
    ticket: LaunchTicket,
) -> Result<(), RegistryError> {
    if stopped(context) || shared.admission.load(Ordering::Acquire) == CLOSED {
        return Err(RegistryError::invalid("creation_canceled_before_effect"));
    }
    let family = state.family.as_mut().ok_or_else(|| RegistryError::invalid("creation_owner"))?;
    family.adopt_fixture_events(input)?;
    family.adopt_lifecycle(input)?;
    family.adopt_diagnostic(input)?;
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
            shared.port.enroll_diagnostic(family, key, ticket.process)?
        }
        FamilyBinding::BackendCustody(key) => {
            shared.port.enroll_backend_custody(family, key, ticket.process)?
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
pub(super) fn bind(permit: ProcessWatchPermit, input: WatchInput) -> Option<WatchJob> {
    match input {
        WatchInput::Native(handle) => Some(permit.bind(handle)),
        #[cfg(test)]
        WatchInput::Simulated => Some(permit.cancel_unbound()),
    }
}
fn stopped(context: &JobContext) -> bool {
    context.cancellation_requested()
        || context.deadline().is_some_and(|deadline| context.now() >= deadline)
}
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
