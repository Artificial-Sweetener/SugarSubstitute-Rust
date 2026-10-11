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
        if binding.owner() != self.owner {
            return Err(RegistryAdmissionError::ForeignOwner);
        }
        if let Some(launch) = &input.lifecycle
            && (binding != FamilyBinding::Desktop(launch.run)
                || input.fixture_events.is_some()
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
            || creation.reservation_bytes < input.required_reservation_bytes()
        {
            return Err(RegistryAdmissionError::InvalidSpecification);
        }
        let desktop_count = self
            .records
            .iter()
            .flatten()
            .filter(|record| record.ticket.binding.desktop() == binding.desktop())
            .count();
        if desktop_count == 64 {
            return Err(RegistryAdmissionError::Saturated);
        }
        // A current run/resource cannot overlap a retained uncertain predecessor.
        if self.records.iter().flatten().any(|record| match (record.ticket.binding, binding) {
            (FamilyBinding::Desktop(old), FamilyBinding::Desktop(new)) => {
                old.instance == new.instance
            }
            (FamilyBinding::Managed(old), FamilyBinding::Managed(new)) => old.id == new.id,
            _ => false,
        }) {
            return Err(RegistryAdmissionError::Busy);
        }
        let index = self
            .records
            .iter()
            .position(Option::is_none)
            .ok_or(RegistryAdmissionError::Saturated)?;
        let process_id = ProcessId::new(self.next_process)
            .map_err(|_| RegistryAdmissionError::IdentityExhausted)?;
        let next =
            self.next_process.checked_add(1).ok_or(RegistryAdmissionError::IdentityExhausted)?;
        let correlation = NativeCorrelation::new(process_id.get())
            .map_err(|_| RegistryAdmissionError::InvalidSpecification)?;
        let native_scope = execution
            .create_native_scope(native_scope_spec, None)
            .map_err(RegistryAdmissionError::Execution)?;
        let root_spec = (native_scope.id(), root_watch.id);
        let permit = execution
            .reserve_process_watch(&native_scope, root_watch, correlation)
            .map_err(RegistryAdmissionError::Execution)?;
        let shared = Arc::new(Shared {
            admission: AtomicU8::new(OPEN),
            native: Mutex::new(NativeState {
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
        });
        let ticket =
            LaunchTicket { process: ProcessKey { owner: self.owner, id: process_id }, binding };
        let mut record = Record {
            ticket,
            shared: shared.clone(),
            native_scope,
            rejected_launch: false,
            creation: None,
            root_spec,
            root_watch: None,
            root_notice: false,
            root: None,
            root_exit: None,
            last_member_exit: None,
            forced: false,
            root_retirement: None,
            member_watch: None,
            member_spec: None,
            member_notice: false,
            watched: None,
            pending_exit: None,
            cleanup: None,
            stop: None,
            retained: 0,
            next: None,
            closed_native: false,
            started: false,
            settled: false,
            error: None,
            unread: [false; 5],
        };
        self.next_process = next;
        let job = execution.submit_blocking(creation_scope, creation, move |context| {
            create(shared, input, context, ticket)
        });
        match job {
            Ok(job) => record.creation = Some(job),
            Err(cause) => {
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
    shared: Arc<Shared>,
    input: WindowsLaunchInput,
    context: JobContext,
    ticket: LaunchTicket,
) -> Result<(), RegistryError> {
    let bytes = input.required_reservation_bytes() - crate::registry_input::FAMILY_STORAGE_BYTES;
    let claim =
        context.memory().claim(bytes).map_err(|cause| RegistryError::new("input_memory", cause))?;
    let mut charged = ChargedInput { input, _claim: claim };
    let family = Family::new(context.memory())?;
    let mut state = lock(&shared.native);
    state.family = Some(family);
    let result = create_owned(&shared, &mut state, &mut charged.input, &context, ticket);
    if let Err(cause) = &result {
        state.failure = Some(cause.clone());
    }
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
    let root = shared.port.create(family, input)?;
    state.creation = Some(CreationFacts { root, resumed: false });
    let duplicate = shared.port.duplicate(family, 0)?;
    let permit = lock(&shared.permit)
        .take()
        .ok_or_else(|| RegistryError::invalid("root_watch_reservation"))?;
    state.watch = bind(permit, duplicate);
    if let FamilyBinding::Desktop(run) = ticket.binding {
        shared.port.enroll_desktop(family, run, ticket.process)?;
    }
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
