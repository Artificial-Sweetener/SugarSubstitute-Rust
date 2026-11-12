// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Prepare original and candidate metadata under the same actual issued native owner.
use super::*;
impl Token {
    /// Stage both native images before the repository can publish a disposition journal.
    /// Returned data remains inside this owner; partial native stages stay with its token.
    pub(crate) fn prepare(&mut self, context: &JobContext) -> Result<(), Issue> {
        let fields = self.fields.as_mut().ok_or(Issue::Identity)?;
        fields.check(context)?;
        if fields.phase != Phase::Issued
            || fields.state.token == 0
            || fields.state.preparation.is_some()
            || fields.state.facts.is_none()
        {
            return Err(Issue::Occupied);
        }
        let guard = fields.generation.borrow(context).map_err(|cause| {
            fields.state.failures.retain(Failure::Control(ControlCause::Source(cause)));
            Issue::Current
        })?;
        let mut current = ControlInvocation::for_generation(&guard, &mut fields.state.history);
        let key = fields.operation;
        let operation = abi::Operation {
            namespace: *key.owner().namespace.bytes(),
            epoch: *key.owner().epoch.bytes(),
            id: key.id().get(),
        };
        let input = abi::Prepare {
            call: abi::Call {
                version: 1,
                phase: 1,
                lease: fields.state.token,
                original: fields.state.original.counter,
                current: current.source(),
            },
            operation,
        };
        let mut output = abi::PrepareOutput {
            state: abi::Output::empty(),
            valid: 0,
            reserved: 0,
            preparation: abi::Preparation::empty(),
        };
        // SAFETY: Same pinned ABI/token and pre-admitted native stages; current source and
        // output are exclusively borrowed for this synchronous invocation only.
        unsafe { (fields.state.api.prepare)(&input, &mut output) };
        let control = current.take_failure();
        drop(current);
        fields.observe(&output.state, control)?;
        if output.valid > 1 || output.reserved != 0 || output.state.retired != 0 {
            fields.phase = Phase::Quarantined;
            return Err(Issue::Invalid);
        }
        if output.valid == 1 {
            fields.state.preparation = Some(output.preparation);
            if fields.query.as_ref().zip(fields.state.facts.as_ref()).is_none_or(
                |(query, source)| {
                    !validation::preparation(&output.preparation, operation, query, source)
                },
            ) {
                fields.phase = Phase::Quarantined;
                return Err(Issue::Invalid);
            }
        }
        if output.state.status != 0 {
            return Err(Issue::Native);
        }
        if output.valid != 1 {
            fields.phase = Phase::Quarantined;
            return Err(Issue::Invalid);
        }
        Ok(())
    }
}
