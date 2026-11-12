// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Coordinate the upstream worker and translate its progress into installer events.
use super::*;
mod presentation;

impl Controller {
    pub(super) fn worker_failed(&mut self, cause: Box<dyn Error>) -> Result<()> {
        tracing::error!(operation="installer_worker_failure", cause_chain=%crate::diagnostics::CauseChain(cause.as_ref()), "Provisioning stopped");
        if let Some(worker) = &mut self.worker {
            worker.close()?;
        }
        if self.unavailable {
            return Ok(());
        }
        self.unavailable = true;
        self.waiting = None;
        self.pending = None;
        self.install_id = None;
        if let Some(storage) = &self.storage {
            storage.cancel_preparation();
        }
        self.prepared = None;
        self.diagnostic(crate::failure_details::Details(cause.as_ref()).to_string())?;
        self.emit(
            None,
            Kind::Unavailable(
                sugarsubstitute_contracts::localization::installer::messages(&self.locale)
                    .worker_stopped
                    .into(),
            ),
        )
    }
    /// Start the scoped upstream worker on demand; native maintenance needs no worker process.
    pub(super) fn start_worker(&mut self) -> Result<()> {
        if self.worker.is_some() {
            return Ok(());
        }
        let bundle = self.bundle.as_ref().ok_or("Installer package is not ready")?;
        let cache = self.cache.as_ref().ok_or("Installer cache scope is absent")?;
        let client = self.client.as_ref().ok_or("Installer client absent")?;
        let scope = self.create_scope(self.scope.as_ref())?;
        self.worker = Some(Worker::start(
            client.clone(),
            scope,
            self.jobs.clone(),
            self.scopes.clone(),
            bundle.clone(),
            cache.clone(),
            self.workspace.clone(),
        )?);
        Ok(())
    }
    pub(super) fn forward(
        &mut self,
        id: String,
        operation: &str,
        input: Value,
        purpose: Purpose,
    ) -> Result<()> {
        tracing::info!(operation="installer_worker_request", request_id=%id, worker_operation=operation, "Submitting provisioning operation");
        if self.pending.is_some() {
            return Err("Provisioning is already active".into());
        }
        self.worker.as_mut().ok_or("Provisioning is preparing")?.send(&json!({
            "version":1,"id":id,"operation":operation,"input":input,
        }))?;
        self.pending = Some(Pending { id, purpose });
        Ok(())
    }
    pub(super) fn worker_event(&mut self, mut value: Value) -> Result<()> {
        let event = value["event"].as_str().ok_or("Worker event missing")?.to_owned();
        crate::diagnostics::worker::observe(
            &event,
            self.pending.as_ref().map(|pending| pending.id.as_str()),
        )?;
        if value["data"]["statusCode"] == "cacheFull" {
            self.worker_failed(message(&value["data"])?.into())?;
            return Ok(());
        }
        if matches!(event.as_str(), "fatal" | "protocol_error") {
            return Err("Provisioning worker failed".into());
        }
        if event == "diagnostic" {
            return self.emit(None, Kind::Diagnostic(events::Diagnostic::project(&value["data"])?));
        }
        let Some(pending) = self.pending.as_ref() else {
            return Err("Worker returned an unsolicited operation result".into());
        };
        if value["id"].as_str() != Some(&pending.id) {
            return Err("Worker request identity changed".into());
        }
        if event == "progress" {
            let mut data = value["data"].clone();
            presentation::progress(&mut data, &self.locale)?;
            let status =
                data["status"].as_str().ok_or("Worker progress status missing")?.to_owned();
            return self.emit(
                Some(pending.id.clone()),
                Kind::Progress(Progress {
                    status,
                    overall: progress(&data),
                    phase: data["phase"].as_str().map(str::to_owned),
                    percent: data["percent"].as_f64(),
                }),
            );
        }
        let pending = self.pending.take().ok_or("Worker request missing")?;
        if !matches!(event.as_str(), "completed" | "failed" | "cancelled") {
            return Err("Unknown worker result".into());
        }
        if event != "completed" {
            self.install_id = None;
            self.prepared = None;
            self.integration = None;
            self.diagnostic(message(&value["data"])?)?;
            presentation::failure(&mut value["data"], &self.locale, event == "cancelled")?;
            return self.emit(
                Some(pending.id),
                if event == "failed" {
                    Kind::Failed(message(&value["data"])?)
                } else {
                    Kind::Cancelled(value["data"]["message"].as_str().map(str::to_owned))
                },
            );
        }
        if matches!(pending.purpose, Purpose::Provision) {
            let runtime: RuntimePaths = serde_json::from_value(value["data"].clone())?;
            let prepared = self.prepared.take().ok_or("Prepared installation missing")?;
            self.submit(StorageWork::Prepare, move |context| {
                let integration = prepared.integration(runtime, &context)?;
                Ok(StorageResult::Integration(prepared, Box::new(integration)))
            })
        } else if matches!(pending.purpose, Purpose::Integrate) {
            let runtime: RuntimePaths = serde_json::from_value(value["data"].clone())?;
            let integration =
                self.integration.take().ok_or("Native integration custody missing")?;
            integration.validate_result(&runtime)?;
            let prepared = self.prepared.take().ok_or("Prepared installation missing")?;
            self.complete_candidate(prepared, runtime)
        } else {
            let completed = match pending.purpose {
                Purpose::Options => Completed::Options(events::options(value["data"].take())?),
                Purpose::Inspect => {
                    let _inspected: RuntimePaths = serde_json::from_value(value["data"].take())?;
                    Completed::Validated
                }
                Purpose::Provision | Purpose::Integrate => {
                    return Err("Unexpected worker completion purpose".into());
                }
            };
            self.emit(Some(pending.id), Kind::Completed(completed))
        }
    }
}

fn progress(data: &Value) -> f64 {
    let (start, span) = match data["phase"].as_str().unwrap_or_default() {
        "download" => (10.0, 40.0),
        "extract" => (50.0, 15.0),
        "setup" => (65.0, 15.0),
        "cleanup" => (80.0, 2.0),
        "update" => (82.0, 10.0),
        "deps" => (85.0, 7.0),
        "verify" => (92.0, 3.0),
        "integration" => (95.0, 3.0),
        _ => (10.0, 0.0),
    };
    start
        + span
            * data["percent"]
                .as_f64()
                .filter(|value| value.is_finite())
                .unwrap_or(0.0)
                .clamp(0.0, 100.0)
            / 100.0
}

/// The wire boundary requires original diagnostic text rather than silently discarding a cause.
fn message(data: &Value) -> Result<String> {
    Ok(data["message"].as_str().ok_or("Worker diagnostic message missing")?.to_owned())
}
