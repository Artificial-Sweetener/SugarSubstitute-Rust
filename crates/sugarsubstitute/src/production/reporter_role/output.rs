// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Encode actual native facts on finite Core workers without allocating another transport buffer.
use sugarsubstitute_contracts::{execution::*, incident::*};
use sugarsubstitute_crash::diagnostic_client::*;
use sugarsubstitute_desktop::report::SaveSelection;
use sugarsubstitute_execution::{ChargedBlockingInput, purpose_pipe::LifecyclePipeWriteBuffer};
pub(super) enum Intent {
    Presented,
    Replaced,
    Action(ReportAction),
    Copy(DiagnosticActionResult),
    Save(SaveSelection),
    Cancelled,
    Retired,
    Failed(ReportFailure),
}
pub(super) struct OutputInput {
    pub(super) key: DiagnosticKey,
    pub(super) request: DiagnosticRequestId,
    pub(super) binding: ReportBinding,
    pub(super) until: Moment,
    pub(super) counter: DiagnosticDeadline,
    pub(super) buffer: Option<LifecyclePipeWriteBuffer>,
    pub(super) intent: Intent,
}
pub(super) struct EncodedOutput {
    pub(super) input: ChargedBlockingInput<OutputInput>,
    pub(super) length: usize,
}
pub(super) struct OutputFailure {
    pub(super) cause: DiagnosticWireError,
    pub(super) input: ChargedBlockingInput<OutputInput>,
}
impl std::fmt::Debug for OutputFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OutputFailure")
            .field("cause", &self.cause)
            .field("binding", &self.input.input().binding)
            .finish_non_exhaustive()
    }
}
impl std::fmt::Display for OutputFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Reporter output encoding failed")
    }
}
impl std::error::Error for OutputFailure {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.cause)
    }
}
/// Keep the exact failed terminal and its original worker funding through error delivery.
pub(super) struct RetainedOutputFailure(pub(super) JobCompletion<EncodedOutput, OutputFailure>);
impl std::fmt::Debug for RetainedOutputFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RetainedOutputFailure")
            .field("terminal", &self.0.outcome.kind())
            .finish_non_exhaustive()
    }
}
impl std::fmt::Display for RetainedOutputFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Reporter output worker stopped")
    }
}
impl std::error::Error for RetainedOutputFailure {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match &self.0.outcome {
            JobOutcome::Failure(cause) => Some(cause),
            _ => None,
        }
    }
}
/// The complete charged input remains returned on semantic refusal or deadline rejection.
pub(super) fn encode_output(
    mut input: ChargedBlockingInput<OutputInput>,
    context: JobContext,
) -> Result<EncodedOutput, OutputFailure> {
    let result = encode_input(input.input_mut(), &context);
    match result {
        Ok(length) => Ok(EncodedOutput { input, length }),
        Err(cause) => Err(OutputFailure { cause, input }),
    }
}
fn encode_input(
    input: &mut OutputInput,
    context: &JobContext,
) -> Result<usize, DiagnosticWireError> {
    if context.deadline() != Some(input.until)
        || context.now() >= input.until
        || context.cancellation_requested()
    {
        return Err(DiagnosticWireError::Time);
    }
    let mut selected = [0_u8; SaveSelection::MAX_UNITS * 2];
    let session = input.binding.session();
    let command = match &input.intent {
        Intent::Presented => DiagnosticCommand::Presented(session),
        Intent::Replaced => DiagnosticCommand::ViewReplaced(session),
        Intent::Action(action) => DiagnosticCommand::Action { session, action: *action },
        Intent::Copy(result) => DiagnosticCommand::CopyResult { session, result: *result },
        Intent::Cancelled => DiagnosticCommand::SaveSelectionCancelled(session),
        Intent::Retired => DiagnosticCommand::NativeRetired(session),
        Intent::Failed(failure) => DiagnosticCommand::ReporterFailed { session, failure: *failure },
        Intent::Save(selection) => {
            if selection.binding() != input.binding
                || selection.destination().len() > SaveSelection::MAX_UNITS
            {
                return Err(DiagnosticWireError::Identity);
            }
            for (unit, bytes) in selection.destination().iter().zip(selected.chunks_exact_mut(2)) {
                bytes.copy_from_slice(&unit.to_le_bytes());
            }
            DiagnosticCommand::SaveSelectionReady {
                session,
                selection: DiagnosticSelection {
                    id: DiagnosticRequestId::new(selection.request().get())?,
                    selection: selection.content(),
                    overwrite: selection.overwrite_consented(),
                    destination: &selected[..selection.destination().len() * 2],
                },
            }
        }
    };
    let frame = DiagnosticFrame {
        key: input.key,
        request: input.request,
        context: DiagnosticContext {
            incident: Some(DiagnosticIncident {
                key: input.binding.incident(),
                revision: input.binding.revision(),
            }),
            deadline: matches!(input.intent, Intent::Copy(_)).then_some(input.counter),
            ..DiagnosticContext::default()
        },
        command,
    };
    encode(frame, input.buffer.as_mut().ok_or(DiagnosticWireError::Incomplete)?.as_mut_bytes())
}
