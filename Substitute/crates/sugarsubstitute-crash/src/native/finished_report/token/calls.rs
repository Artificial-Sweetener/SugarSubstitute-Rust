// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Query and stream only the same live generation and issued token.
use super::*;
impl Token {
    /// Query only the real pinned generation, full subject and exact bounded build.
    /// Issuance precedes ABI entry; even zero output remains issued until a real retired result.
    pub(crate) fn open(
        &mut self,
        key: IncidentKey,
        build: &BuildIdentity,
        dump: Option<DumpId>,
        context: &JobContext,
    ) -> Result<u32, Issue> {
        let fields = self.fields.as_mut().ok_or(Issue::Identity)?;
        fields.check(context)?;
        if fields.phase != Phase::Unused || !fields.counter_ready || fields.cpp.is_none() {
            return Err(Issue::Occupied);
        }
        let guard = fields.generation.borrow(context).map_err(|cause| {
            fields.state.failures.retain(Failure::Control(ControlCause::Source(cause)));
            Issue::Current
        })?;
        let owner = guard.coordinator().map_err(|cause| {
            fields.state.failures.retain(Failure::Control(ControlCause::Source(cause)));
            Issue::Current
        })?;
        let origin = guard.origin().map_err(|cause| {
            fields.state.failures.retain(Failure::Control(ControlCause::Source(cause)));
            Issue::Current
        })?;
        let relation = match origin {
            sugarsubstitute_contracts::crash::CaptureGenerationOrigin::Coordinator(actual) => {
                actual == owner
                    && owner == key.subject().coordinator()
                    && fields.operation.owner() == owner
            }
            sugarsubstitute_contracts::crash::CaptureGenerationOrigin::Recovery(_) => {
                fields.operation == guard.current_operation()
                    && fields.operation.owner().namespace == origin.coordinator().namespace
                    && origin.capture_key().is_ok_and(|key| key == owner)
                    && origin.contains(key.subject()).is_ok_and(|matching| matching)
            }
        };
        if !relation {
            return Err(Issue::Identity);
        }
        let recovery = abi::RecoveryOrigin::from_origin(origin, fields.operation)
            .map_err(|_| Issue::Identity)?;
        let identities = guard.identities().map_err(|cause| {
            fields.state.failures.retain(Failure::Control(ControlCause::Source(cause)));
            Issue::Current
        })?;
        let database = guard
            .database()
            .map_err(|cause| {
                fields.state.failures.retain(Failure::Control(ControlCause::Source(cause)));
                Issue::Current
            })?
            .as_raw_handle() as usize;
        let metrics = guard
            .metrics()
            .map_err(|cause| {
                fields.state.failures.retain(Failure::Control(ControlCause::Source(cause)));
                Issue::Current
            })?
            .as_raw_handle() as usize;
        let mut annotation = [0; 64];
        annotation[..build.as_str().len()].copy_from_slice(build.as_str().as_bytes());
        let mut current = ControlInvocation::for_generation(&guard, &mut fields.state.history);
        let input = abi::Open {
            version: 1,
            reserved: 0,
            database,
            metrics,
            database_identity: identities[0],
            metrics_identity: identities[1],
            subject: subject(key),
            build_length: build.as_str().len() as u32,
            build: annotation,
            optional_uuid: dump.map_or([0; 16], |id| *id.bytes()),
            has_uuid: u32::from(dump.is_some()),
            original: fields.state.original.counter,
            current: current.source(),
        };
        let mut output = abi::Output::empty();
        fields.query = Some(validation::Query::new(&input));
        fields.phase = Phase::Issued;
        // SAFETY: genuine pinned API, pre-admitted fixed owner, live exact generation handles
        // and synchronous exclusive control/output borrows remain valid until return.
        if let Some(origin) = recovery {
            let key = fields.operation;
            let input = abi::Open2 {
                version: 2,
                reserved: 0,
                origin,
                operation: abi::Operation {
                    namespace: *key.owner().namespace.bytes(),
                    epoch: *key.owner().epoch.bytes(),
                    id: key.id().get(),
                },
                query: input,
            };
            // SAFETY: genuine Root-local pins, exact origin and original current source stay borrowed.
            unsafe { (fields.state.api.open2)(&input, &mut output) };
        } else {
            // SAFETY: strict original Coordinator identity and v1 borrowed owners are unchanged.
            unsafe { (fields.state.api.open)(&input, &mut output) };
        }
        let control = current.take_failure();
        drop(current);
        fields.observe(&output, control)?;
        if (output.status == 0 && (output.facts_valid != 1 || output.lease == 0))
            || (output.retired == 1
                && (!matches!(output.status, 6 | 7)
                    || output.primary_error == 0
                    || output.facts_valid != 0))
        {
            fields.phase = Phase::Quarantined;
            return Err(Issue::Invalid);
        }
        fields.accept_retirement(&output);
        Ok(output.status)
    }
    /// Read directly from the retained native source into the caller's already-funded buffer.
    pub(crate) fn read(
        &mut self,
        offset: u64,
        output: &mut [u8],
        context: &JobContext,
    ) -> Result<usize, Issue> {
        let fields = self.fields.as_mut().ok_or(Issue::Identity)?;
        fields.check(context)?;
        if fields.phase != Phase::Issued || fields.state.token == 0 || output.len() > 64 * 1024 {
            return Err(Issue::Identity);
        }
        let guard = fields.generation.borrow(context).map_err(|cause| {
            fields.state.failures.retain(Failure::Control(ControlCause::Source(cause)));
            Issue::Current
        })?;
        let mut current = ControlInvocation::for_generation(&guard, &mut fields.state.history);
        let input = abi::Read {
            call: abi::Call {
                version: 1,
                phase: 1,
                lease: fields.state.token,
                original: fields.state.original.counter,
                current: current.source(),
            },
            offset,
            output: output.as_mut_ptr(),
            capacity: output.len() as u32,
            reserved: 0,
        };
        let mut result = abi::Output::empty();
        // SAFETY: exact same retained token and live bounded mutable output/control borrows.
        unsafe { (fields.state.api.read)(&input, &mut result) };
        let control = current.take_failure();
        drop(current);
        fields.observe(&result, control)?;
        let bytes = usize::try_from(result.read_bytes).map_err(|_| Issue::Invalid)?;
        if bytes > output.len() || result.retired != 0 {
            fields.phase = Phase::Quarantined;
            return Err(Issue::Invalid);
        }
        if result.status != 0 {
            return Err(Issue::Native);
        }
        Ok(bytes)
    }
}

pub(super) fn subject(key: IncidentKey) -> abi::Subject {
    let owner = key.subject().coordinator();
    let (tag, instance, generation) = match key.subject() {
        IncidentSubject::Desktop(run) => (1, run.instance.get(), run.generation.get()),
        IncidentSubject::Supervisor(_) => (2, 0, 0),
    };
    abi::Subject {
        tag,
        namespace: *owner.namespace.bytes(),
        epoch: *owner.epoch.bytes(),
        reserved: 0,
        instance,
        generation,
    }
}
