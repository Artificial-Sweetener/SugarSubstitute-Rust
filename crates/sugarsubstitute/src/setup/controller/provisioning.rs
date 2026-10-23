// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Coordinate the upstream worker and translate its progress into installer events.
use super::*;

impl Controller {
    pub(super) fn worker_failed(&mut self, cause: Box<dyn Error>) -> Result<()> {
        tracing::error!(operation="installer_worker_failure", cause=%cause, "Provisioning stopped");
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
        self.reply(None, "unavailable", json!({"message":cause.to_string()}))
    }
    /// Start the scoped upstream worker on demand; native maintenance needs no worker process.
    pub(super) fn start_worker(&mut self) -> Result<()> {
        if self.worker.is_some() {
            return Ok(());
        }
        let bundle = self.bundle.as_ref().ok_or("Installer package is not ready")?;
        let cache = self.cache.as_ref().ok_or("Installer cache scope is absent")?;
        let client = self.client.as_ref().ok_or("Installer client absent")?;
        let scope = client.create_scope(
            ScopeSpec { id: ScopeId::new(2)?, deadline: None },
            self.scope.as_ref(),
        )?;
        self.worker = Some(Worker::start(
            client.clone(),
            scope,
            self.jobs.clone(),
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
        self.worker.as_ref().ok_or("Provisioning is preparing")?.send(&json!({
            "version":1,"id":id,"operation":operation,"input":input,
        }))?;
        self.pending = Some(Pending { id, purpose });
        Ok(())
    }
    pub(super) fn worker_event(&mut self, value: Value) -> Result<()> {
        let event = value["event"].as_str().ok_or("Worker event missing")?;
        tracing::debug!(operation="installer_worker_event", event, request_id=?value["id"].as_str(), "Received provisioning event");
        if matches!(event, "fatal" | "protocol_error") {
            return Err("Provisioning worker failed".into());
        }
        if event == "diagnostic" {
            return self.reply(None, event, value["data"].clone());
        }
        let Some(pending) = self.pending.as_ref() else {
            return Err("Worker returned an unsolicited operation result".into());
        };
        if value["id"].as_str() != Some(&pending.id) {
            return Err("Worker request identity changed".into());
        }
        if event == "progress" {
            let mut data = value["data"].clone();
            if let Some(code) = data["statusCode"].as_str() {
                let copy =
                    sugarsubstitute_contracts::localization::installer::messages(&self.locale);
                data["status"] = json!(copy.get(code).ok_or("Unknown installer progress message")?);
            }
            data["overall"] = json!(progress(&data));
            return self.reply(None, event, data);
        }
        let pending = self.pending.take().ok_or("Worker request missing")?;
        if !matches!(event, "completed" | "failed" | "cancelled") {
            return Err("Unknown worker result".into());
        }
        if event != "completed" {
            self.install_id = None;
            self.prepared = None;
            return self.reply(Some(&pending.id), event, value["data"].clone());
        }
        if matches!(pending.purpose, Purpose::Provision) {
            let runtime: RuntimePaths = serde_json::from_value(value["data"].clone())?;
            let mut input = serde_json::to_value(runtime)?;
            input["models"] =
                json!(self.selection.as_ref().ok_or("Installation choices absent")?.models);
            self.forward(pending.id, "integrate", input, Purpose::Integrate)
        } else if matches!(pending.purpose, Purpose::Integrate) {
            let runtime: RuntimePaths = serde_json::from_value(value["data"].clone())?;
            let prepared = self.prepared.take().ok_or("Prepared installation missing")?;
            self.complete_candidate(prepared, runtime)
        } else {
            self.reply(Some(&pending.id), event, value["data"].clone())
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
