// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Transfer complete Core-encoded output and observe actual write completion before retiring its owner.
use super::*;
impl ReporterSession {
    pub(super) fn consume_output(&mut self) -> Result<(), ReportSurfaceError> {
        if self.output_failed {
            return Ok(());
        }
        if let Some(actual) = self.channel.take_written()
            && self.written == Some(actual)
        {
            self.written = None;
            self.encoded = None;
        }
        if let Some(job) = &mut self.encoding
            && let Some(done) = job.try_take_completion()
        {
            self.encoded = Some(done);
            self.encoding = None;
        }
        if self.encoded.as_ref().is_some_and(|done| !matches!(done.outcome, JobOutcome::Success(_)))
        {
            self.output_failed = true;
            if let Some(done) = self.encoded.take() {
                return Err(ReportSurfaceError::new(
                    "reporter_encoding_failed",
                    RetainedOutputFailure(done),
                ));
            }
        }
        if self.written.is_none()
            && let Some(done) = &mut self.encoded
        {
            let JobOutcome::Success(encoded) = &mut done.outcome else {
                return Err(Self::rejected("reporter_encoding_identity"));
            };
            let input = encoded.input.input_mut();
            if let Some(buffer) = input.buffer.take() {
                match self.channel.write_encoded(input.until, buffer, encoded.length) {
                    Ok(operation) => self.written = Some(operation),
                    Err(cause) => {
                        input.buffer = Some(cause.buffer);
                        self.output_failed = true;
                        return Err(ReportSurfaceError::new("reporter_output_write", cause.error));
                    }
                }
            }
        }
        if self.encoding.is_none() && self.encoded.is_none() && self.written.is_none() {
            let index = self
                .outgoing
                .iter()
                .enumerate()
                .filter_map(|(index, input)| {
                    input.as_ref().map(|input| (index, input.request.get()))
                })
                .min_by_key(|(_, request)| *request)
                .map(|(index, _)| index);
            if let Some(index) = index {
                let input = self.outgoing[index]
                    .as_mut()
                    .ok_or_else(|| Self::rejected("reporter_output_missing"))?;
                if input.buffer.is_none() {
                    input.buffer = self.channel.take_output().map_err(|cause| {
                        ReportSurfaceError::new("reporter_output_buffer", cause)
                    })?;
                }
                if input.buffer.is_some() {
                    let client = self
                        .client
                        .as_ref()
                        .ok_or_else(|| Self::rejected("reporter_execution_missing"))?;
                    let finite = self
                        .finite
                        .as_ref()
                        .ok_or_else(|| Self::rejected("reporter_scope_missing"))?;
                    let spec = JobSpec {
                        id: self.jobs.allocate().map_err(|cause| {
                            ReportSurfaceError::new("reporter_output_job", cause)
                        })?,
                        operation: Operation::NativeLifecycle,
                        class: JobClass::InfrastructureLifecycle,
                        deadline: Some(input.until),
                        reservation_bytes: 64 * 1024,
                        replacement: None,
                    };
                    match client.submit_blocking_owned_box(
                        finite,
                        spec,
                        &mut self.outgoing[index],
                        encode_output,
                    ) {
                        Ok(job) => self.encoding = Some(job),
                        Err(cause) => {
                            self.output_failed = true;
                            let original = self
                                .funding
                                .as_ref()
                                .ok_or_else(|| Self::rejected("reporter_output_funding_missing"))?
                                .ui
                                .clone();
                            return Err(ReportSurfaceError::new(
                                "reporter_output_admission",
                                OwnedOutputAdmissionFailure { cause, _original: original },
                            ));
                        }
                    }
                }
            }
        }
        Ok(())
    }
}

/// Retain the exact rejection and original finite credit through checked child disposal.
#[derive(Debug)]
pub(super) struct OwnedOutputAdmissionFailure {
    cause: sugarsubstitute_execution::OwnedInputError,
    _original: MemoryLease,
}
impl std::fmt::Display for OwnedOutputAdmissionFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Reporter output admission failed")
    }
}
impl std::error::Error for OwnedOutputAdmissionFailure {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.cause)
    }
}
#[cfg(test)]
#[path = "output/admission_tests.rs"]
mod admission_tests;
