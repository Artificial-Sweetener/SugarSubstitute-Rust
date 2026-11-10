// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Route bounded metadata transfers through the exact provisioning family's native cache lease.
pub(super) mod session;
use super::*;
use serde::Deserialize;
use serde_json::{Value, json};
use session::Session;
use std::io;
use sugarsubstitute_supervision::setup_storage::{MetadataRecord, ProvisioningMetadata};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Envelope {
    version: u32,
    serial: u64,
    operation: Request,
}
#[derive(Deserialize)]
#[serde(tag = "op", rename_all = "snake_case", deny_unknown_fields)]
enum Request {
    Read { name: MetadataRecord },
    Next,
    Begin { name: MetadataRecord, length: u64 },
    Chunk { offset: u64, hex: String },
    Commit,
    Abort,
}
impl Request {
    /// Validate fixed records and finite extents before execution admission.
    fn validate(&self) -> io::Result<()> {
        match self {
            Self::Begin { length, .. } if *length > 256 * 1024 * 1024 => return Err(invalid()),
            Self::Chunk { offset, hex }
                if *offset > 256 * 1024 * 1024
                    || hex.is_empty()
                    || hex.len() > 16384
                    || hex.len() % 2 != 0
                    || !hex.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)) =>
            {
                return Err(invalid());
            }
            _ => {}
        }
        Ok(())
    }
}
/// Inert original command matching facts; native authority comes from the prepared launch.
pub(super) struct MetadataScope {
    request: String,
    serial: u64,
}
impl MetadataScope {
    /// Match inert wire facts before admitting any native cache operation.
    fn decode(&self, value: &Value) -> Result<Envelope> {
        if value["version"] != 1 || value["id"].as_str() != Some(&self.request) {
            return Err("Foreign metadata operation".into());
        }
        let request: Envelope = serde_json::from_value(value["data"].clone())?;
        if request.version != 1
            || self.serial.checked_add(1) != Some(request.serial)
            || request.serial > 9_007_199_254_740_991
        {
            return Err("Metadata sequence changed".into());
        }
        request.operation.validate()?;
        Ok(request)
    }
}
/// Native state and encoded replies retain actual allocation through completion delivery.
/// Setup probes for files that may not exist yet; the page receives the absence as its answer, so
/// it is reported as an expected outcome rather than a failure.
fn answers_absence(cause: &io::Error) -> bool {
    cause.kind() == io::ErrorKind::NotFound
}
pub(super) struct MetadataCompletion {
    state: Session,
    request: String,
    serial: u64,
    result: io::Result<Value>,
    _memory: sugarsubstitute_contracts::execution::MemoryClaim,
}
impl Worker {
    /// Admit only the next operation from the current host command and original native cache grant.
    pub(super) fn metadata_event(&mut self, value: &Value) -> Result<bool> {
        if value["event"] != "metadata" {
            if matches!(value["event"].as_str(), Some("completed" | "failed" | "cancelled"))
                && self
                    .metadata_scope
                    .as_ref()
                    .is_some_and(|s| value["id"].as_str() == Some(&s.request))
            {
                if self.metadata_io.is_some() {
                    return Err("Worker finished before metadata IO".into());
                }
                self.metadata_scope = None;
                self.metadata_state = None;
            }
            return Ok(false);
        }
        let scope = self.metadata_scope.as_mut().ok_or("Unsolicited metadata operation")?;
        if self.metadata_io.is_some() {
            return Err("Foreign or overlapping metadata operation".into());
        }
        let request = scope.decode(value)?;
        scope.serial = request.serial;
        let identity = scope.request.clone();
        let access = self.metadata_access.as_ref().ok_or("Metadata native grant absent")?.clone();
        let mut state = self.metadata_state.take().unwrap_or_default();
        let mut specification = self.spec(
            JobClass::InfrastructureOrdinary,
            10 * 1024 * 1024,
            Some(self.client.now()?.checked_add(Duration::from_secs(30))?),
        )?;
        specification.operation = Operation::Infrastructure;
        self.metadata_io = Some(self.client.submit_blocking(&self.scope, specification, move |context| {
            let memory = context.memory().claim(2 * 1024 * 1024).map_err(io::Error::other)?;
            let result = state.perform(&access, request.operation, &context);
            match &result {
                Err(cause) if answers_absence(cause) => {
                    tracing::debug!(operation="worker_metadata_io", request_id=%identity, serial=request.serial, "Native metadata found no entry");
                }
                Err(cause) => {
                    tracing::warn!(operation="worker_metadata_io", request_id=%identity, serial=request.serial, cause_chain=%crate::diagnostics::CauseChain(cause), "Native metadata operation failed");
                }
                Ok(_) => {}
            }
            Ok(MetadataCompletion { state, request: identity, serial: request.serial, result, _memory: memory })
        })?);
        Ok(true)
    }
    /// Observe actual native IO completion before replying or allowing cache finalization.
    pub(super) fn poll_metadata_io(&mut self) -> Result<()> {
        let Some(job) = &mut self.metadata_io else {
            return Ok(());
        };
        let Some(completion) = job.try_take_completion() else {
            return Ok(());
        };
        self.metadata_io = None;
        let completed = match completion.outcome {
            JobOutcome::Success(completed) => completed,
            JobOutcome::Cancelled if self.closing => return Ok(()),
            JobOutcome::Failure(cause) => return Err(cause.into()),
            _ => return Err("Native metadata job did not complete".into()),
        };
        if self.closing {
            return Ok(());
        }
        if !self
            .metadata_scope
            .as_ref()
            .is_some_and(|s| s.request == completed.request && s.serial == completed.serial)
        {
            return Err("Native metadata result outlived its command".into());
        }
        let result = match completed.result {
            Ok(data) => json!({"ok":true,"data":data}),
            Err(cause) => {
                let code = match cause.kind() {
                    io::ErrorKind::NotFound => "ENOENT",
                    io::ErrorKind::AlreadyExists => "EEXIST",
                    io::ErrorKind::PermissionDenied => "EACCES",
                    io::ErrorKind::WouldBlock => "EBUSY",
                    io::ErrorKind::StorageFull => {
                        self.pressure.metadata = true;
                        "ENOSPC"
                    }
                    io::ErrorKind::Interrupted => "EINTR",
                    io::ErrorKind::TimedOut => "ETIMEDOUT",
                    _ => "EIO",
                };
                json!({"ok":false,"error":{"code":code,"osCode":cause.raw_os_error()}})
            }
        };
        self.transport.as_ref().ok_or("Metadata transport absent")?.send(serde_json::to_string(&json!({
            "version":1,"id":completed.request,"operation":"metadata_reply",
            "input":{"version":1,"serial":completed.serial,"index":0,"done":true,"result":result}
        }))?)?;
        self.metadata_state = Some(completed.state);
        Ok(())
    }
}
fn invalid() -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, "Invalid native metadata operation")
}
/// Validate inert command identity before transport admission; cancellation retains its scope.
pub(super) fn command_scope(value: &Value) -> Result<Option<MetadataScope>> {
    if value["operation"] == "cancel" {
        return Ok(None);
    }
    Ok(Some(MetadataScope {
        request: value["id"].as_str().ok_or("Metadata command identity absent")?.into(),
        serial: 0,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn a_missing_entry_is_an_answer_while_other_metadata_failures_stay_failures() {
        assert!(answers_absence(&io::Error::from(io::ErrorKind::NotFound)));
        assert!(answers_absence(&io::Error::from_raw_os_error(2)));
        assert!(!answers_absence(&io::Error::from(io::ErrorKind::PermissionDenied)));
        assert!(!answers_absence(&io::Error::from_raw_os_error(5)));
    }
    #[test]
    fn metadata_wire_rejects_paths_unbounded_extents_and_noncanonical_chunks() {
        for operation in [
            json!({"op":"read","name":"settings.json"}),
            json!({"op":"read","name":"../fetch-cache.json"}),
            json!({"op":"begin","name":"fetch-cache.json","length":268435457}),
            json!({"op":"read","name":"fetch-cache.json","root":"E:/other"}),
            json!({"op":"chunk","offset":0,"hex":"AB"}),
            json!({"op":"chunk","offset":0,"hex":"a"}),
            json!({"op":"chunk","offset":0,"hex":"00".repeat(8193)}),
            json!({"op":"remove","name":"fetch-cache.json"}),
        ] {
            assert!(
                serde_json::from_value::<Request>(operation.clone())
                    .map_or(true, |request| request.validate().is_err()),
                "accepted {operation}"
            );
        }
    }
    #[test]
    fn original_command_and_next_serial_are_required_before_native_admission() {
        let scope = MetadataScope { request: "original".into(), serial: 4 };
        let valid = json!({"version":1,"id":"original","data":{"version":1,"serial":5,"operation":{"op":"next"}}});
        assert!(scope.decode(&valid).is_ok());
        for changed in [
            json!({"version":1,"id":"foreign","data":valid["data"]}),
            json!({"version":2,"id":"original","data":valid["data"]}),
            json!({"version":1,"id":"original","data":{"version":1,"serial":4,"operation":{"op":"next"}}}),
            json!({"version":1,"id":"original","data":{"version":1,"serial":6,"operation":{"op":"next"}}}),
            json!({"version":1,"id":"original","data":{"version":2,"serial":5,"operation":{"op":"next"}}}),
        ] {
            assert!(scope.decode(&changed).is_err());
        }
    }
}
