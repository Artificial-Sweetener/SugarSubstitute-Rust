// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Admit worker record requests only for the active host-selected managed folder.
mod session;
#[cfg(test)]
mod tests;
mod wire;
use super::*;
pub(super) use session::RecordSession;
use std::io;
use sugarsubstitute_contracts::execution::MemoryClaim;
use wire::{Request, response};

/// The host command, rather than a worker-supplied path, grants this finite record scope.
pub(super) struct RecordScope {
    request: String,
    path: PathBuf,
    writable: bool,
    serial: u64,
}
impl RecordScope {
    /// Decode only the next operation belonging to this exact active host command.
    fn decode(&self, value: &serde_json::Value) -> Result<wire::Envelope> {
        if value["version"] != 1 || value["id"].as_str() != Some(&self.request) {
            return Err("Foreign native storage request".into());
        }
        let request: wire::Envelope = serde_json::from_value(value["data"].clone())?;
        if request.version != 1
            || self.serial.checked_add(1) != Some(request.serial)
            || request.serial > 9_007_199_254_740_991
        {
            return Err("Native storage sequence or version changed".into());
        }
        request.operation.validate()?;
        Ok(request)
    }
}
/// Actual operation output and retained state stay charged through UI-thread delivery.
pub(super) struct RecordCompletion {
    state: RecordSession,
    request: String,
    serial: u64,
    result: io::Result<Vec<serde_json::Value>>,
    _memory: MemoryClaim,
}
impl Worker {
    /// Validate inert authority before transport admission; performs no filesystem work.
    pub(super) fn record_authority(
        &self,
        value: &serde_json::Value,
    ) -> Result<Option<RecordScope>> {
        if value["operation"] == "cancel" {
            return Ok(None);
        }
        if self.record_job.is_some() {
            return Err("Native record operation is still active".into());
        }
        Ok(match value["operation"].as_str() {
            Some(operation @ ("inspect" | "provision")) => Some(RecordScope {
                request: value["id"].as_str().ok_or("Record request identity absent")?.into(),
                path: value["input"]["directory"]
                    .as_str()
                    .ok_or("Selected record folder absent")?
                    .into(),
                writable: operation == "provision",
                serial: 0,
            }),
            _ => None,
        })
    }
    /// Consume a private storage event before it can reach product event handling.
    pub(super) fn record_event(&mut self, value: &serde_json::Value) -> Result<bool> {
        if value["event"] != "storage" {
            if matches!(value["event"].as_str(), Some("completed" | "failed" | "cancelled"))
                && self
                    .record_scope
                    .as_ref()
                    .is_some_and(|scope| value["id"].as_str() == Some(&scope.request))
            {
                if self.record_job.is_some() {
                    return Err("Worker completed before its native storage operation".into());
                }
                self.record_scope = None;
                self.record_state = None;
            }
            return Ok(false);
        }
        let scope = self.record_scope.as_mut().ok_or("Unsolicited native storage request")?;
        if self.record_job.is_some() {
            return Err("Native storage requests cannot overlap".into());
        }
        let request = scope.decode(value)?;
        let serial = request.serial;
        scope.serial = serial;
        let parent = scope.request.clone();
        let state = self
            .record_state
            .take()
            .unwrap_or_else(|| RecordSession::new(scope.path.clone(), scope.writable));
        let mut specification = self.spec(
            JobClass::InfrastructureOrdinary,
            4 * 1024 * 1024,
            Some(self.client.now()?.checked_add(Duration::from_secs(30))?),
        )?;
        specification.operation = Operation::Infrastructure;
        self.record_job =
            Some(self.client.submit_blocking(&self.scope, specification, move |context| {
                let memory = context.memory().claim(2 * 1024 * 1024).map_err(io::Error::other)?;
                let mut state = state;
                let result = state.perform(request.operation, &context);
                Ok(RecordCompletion { state, request: parent, serial, result, _memory: memory })
            })?);
        Ok(true)
    }
    /// Observe the actual finite job outcome before answering the authenticated worker.
    pub(super) fn poll_record_job(&mut self) -> Result<()> {
        let Some(job) = &mut self.record_job else {
            return Ok(());
        };
        let Some(completion) = job.try_take_completion() else {
            return Ok(());
        };
        self.record_job = None;
        let completed = match completion.outcome {
            JobOutcome::Success(completed) => completed,
            JobOutcome::Cancelled if self.closing => return Ok(()),
            JobOutcome::Failure(cause) => return Err(cause.into()),
            _ => return Err("Native storage job did not complete".into()),
        };
        if self.closing {
            return match completed.result {
                Err(cause)
                    if matches!(
                        cause.kind(),
                        io::ErrorKind::Interrupted | io::ErrorKind::TimedOut
                    ) =>
                {
                    tracing::debug!(operation="worker_record_io", request_id=%completed.request, cause=%cause, "Closed record operation observed its controls");
                    Ok(())
                }
                Err(cause) => Err(cause.into()),
                Ok(_) => Ok(()),
            };
        }
        if !self.record_scope.as_ref().is_some_and(|scope| {
            scope.request == completed.request && scope.serial == completed.serial
        }) {
            return Err("Native storage result outlived its original request".into());
        }
        let frames = response(&completed.request, completed.serial, completed.result);
        for frame in frames {
            self.transport
                .as_ref()
                .ok_or("Native storage transport absent")?
                .send(serde_json::to_string(&frame)?)?;
        }
        self.record_state = Some(completed.state);
        Ok(())
    }
}
