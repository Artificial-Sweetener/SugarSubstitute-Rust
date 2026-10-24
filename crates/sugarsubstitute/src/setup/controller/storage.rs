// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Admit native installation phases and consume their owned terminal results.
use super::*;

pub(super) enum StorageResult {
    Bundle(Arc<VerifiedBundle>, setup_storage::cache::RuntimeCache),
    Inspected(PathBuf, Option<Box<setup_storage::InstallationSummary>>),
    Imported(Box<setup_storage::SignedInstallation>),
    Prepared(Box<SignedPreparation>),
    Staged(Box<SignedPreparation>),
    Completed(Box<SignedCandidate>),
    Installed(CompletedInstallation),
    Opened(completion::OpenedLaunch),
    Transferred,
    Launched,
    Retired(CreationOutcome),
}
pub(super) enum StorageWork {
    Metadata,
    Payload,
    Import,
    Prepare,
    Publish,
    Open,
    Transfer,
    Launch,
    Retire,
}
/// Keep cancellation policy with the admitted job whose original outcome must be observed.
pub(super) struct StorageJob {
    pub(super) work: StorageWork,
    pub(super) job: OwnedJob<StorageResult, io::Error>,
}
impl StorageJob {
    /// Transfer, creation and recovery must deliver their actual outcome even after setup closes.
    pub(super) fn cancel_preparation(&self) {
        if !matches!(
            self.work,
            StorageWork::Open | StorageWork::Transfer | StorageWork::Launch | StorageWork::Retire
        ) {
            self.job.cancel();
        }
    }
}
impl Controller {
    pub(super) fn submit(
        &mut self,
        work: StorageWork,
        task: impl FnOnce(JobContext) -> io::Result<StorageResult> + Send + 'static,
    ) -> Result<()> {
        let retirement = matches!(work, StorageWork::Retire);
        if self.storage.is_some() || (self.closing && !retirement) {
            return Err("Installation operation is already active or closing".into());
        }
        let client = self.client.as_ref().ok_or("Installer execution is absent")?;
        let deadline = match work {
            StorageWork::Metadata | StorageWork::Open | StorageWork::Transfer => {
                Some(client.now()?.checked_add(std::time::Duration::from_secs(60))?)
            }
            StorageWork::Launch | StorageWork::Retire => {
                Some(client.now()?.checked_add(std::time::Duration::from_secs(30))?)
            }
            // Protected native effects require a finite phase, including large local archives.
            // Streamed copies still observe cancellation between blocks.
            StorageWork::Payload
            | StorageWork::Import
            | StorageWork::Prepare
            | StorageWork::Publish => {
                Some(client.now()?.checked_add(std::time::Duration::from_secs(30 * 60))?)
            }
        };
        let job = client.submit_blocking(
            self.scope.as_ref().ok_or("Installer scope is absent")?,
            JobSpec {
                id: self.jobs.allocate()?,
                operation: Operation::Infrastructure,
                class: if retirement {
                    JobClass::InfrastructureLifecycle
                } else {
                    JobClass::InfrastructureOrdinary
                },
                deadline,
                reservation_bytes: match work {
                    StorageWork::Payload => setup_storage::MATERIALIZATION_RESERVATION_BYTES,
                    StorageWork::Import => setup_storage::SIGNED_INSTALLER_RESERVATION_BYTES,
                    StorageWork::Prepare => setup_storage::SIGNED_PREPARATION_RESERVATION_BYTES,
                    StorageWork::Transfer => setup_storage::SIGNED_TRANSFER_RESERVATION_BYTES,
                    StorageWork::Publish => setup_storage::SIGNED_TRANSFER_RESERVATION_BYTES,
                    StorageWork::Open => setup_storage::LAUNCH_RESERVATION_BYTES,
                    StorageWork::Launch => {
                        sugarsubstitute_supervision::application_launch::CREATION_RESERVATION_BYTES
                    }
                    StorageWork::Retire => {
                        sugarsubstitute_supervision::application_launch::RECOVERY_RESERVATION_BYTES
                    }
                    _ => setup_storage::RESERVATION_BYTES,
                },
                replacement: None,
            },
            task,
        )?;
        self.storage = Some(StorageJob { work, job });
        Ok(())
    }
    pub(super) fn poll_storage(&mut self) -> Result<()> {
        let Some(storage) = &mut self.storage else {
            return Ok(());
        };
        let Some(completion) = storage.job.try_take_completion() else {
            return Ok(());
        };
        let retirement = matches!(storage.work, StorageWork::Retire);
        self.storage = None;
        if retirement {
            return match completion.outcome {
                JobOutcome::Success(StorageResult::Retired(CreationOutcome::Created)) => {
                    self.launch_completed()
                }
                JobOutcome::Success(StorageResult::Retired(CreationOutcome::Indeterminate)) => {
                    self.relaunch = None;
                    self.installed = None;
                    let cause = self.launch_failure.take().unwrap_or_else(|| {
                        io::Error::other(
                            sugarsubstitute_contracts::localization::installer::messages(
                                &self.locale,
                            )
                            .setup_failed,
                        )
                    });
                    if self.closing || self.unavailable {
                        self.install_id = None;
                        Ok(())
                    } else {
                        self.worker_failed(Box::new(cause))
                    }
                }
                JobOutcome::Success(StorageResult::Retired(CreationOutcome::NotCreated)) => {
                    self.relaunch = None;
                    if let Some(cause) = self.launch_failure.take()
                        && !self.closing
                        && !self.unavailable
                    {
                        self.report_storage_failure(cause)?;
                    }
                    Ok(())
                }
                JobOutcome::Failure(cause) => Err(cause.into()),
                _ => Err("Installer launch recovery ended without a result".into()),
            };
        }
        if self.closing || self.unavailable {
            match completion.outcome {
                JobOutcome::Success(StorageResult::Opened(opened)) => {
                    self.relaunch = Some(opened.into_launch());
                }
                JobOutcome::Failure(cause) => return Err(cause.into()),
                JobOutcome::Success(_) | JobOutcome::Cancelled => {}
                _ => return Err("Installer storage operation ended without a result".into()),
            }
            return Ok(());
        }
        match completion.outcome {
            JobOutcome::Success(StorageResult::Inspected(root, existing)) => {
                let id =
                    self.inspection_id.take().ok_or("Installation inspection identity absent")?;
                self.reply(
                    Some(&id),
                    "completed",
                    json!({
                        "existing":existing, "comfyRoot":root.join("ComfyUI"),
                        "models":root.join("models"), "outputs":root.join("outputs"),
                    }),
                )?;
            }
            JobOutcome::Success(StorageResult::Bundle(bundle, cache)) => {
                self.bundle = Some(bundle);
                self.cache = Some(cache);
            }
            JobOutcome::Success(StorageResult::Imported(imported)) => {
                self.submit(StorageWork::Prepare, move |context| {
                    crate::authority_clock::authorize(|now| {
                        imported.prepare_installation(now, &context)
                    })
                    .map(|prepared| StorageResult::Prepared(Box::new(prepared)))
                })?;
            }
            JobOutcome::Success(StorageResult::Prepared(prepared)) => {
                self.submit(StorageWork::Prepare, move |context| {
                    prepared.stage(&context)?;
                    Ok(StorageResult::Staged(prepared))
                })?;
            }
            JobOutcome::Success(StorageResult::Staged(prepared)) => {
                if let Some(runtime) = prepared.maintenance_runtime() {
                    let runtime = runtime.clone();
                    return self.complete_candidate(prepared, runtime);
                }
                self.prepared = Some(prepared);
                let selection = self.selection.as_ref().ok_or("Installation choices absent")?;
                let id = self.install_id.clone().ok_or("Installation request absent")?;
                if selection.mode == "managed" {
                    self.forward(id, "provision", json!({
                        "directory":selection.comfy_root, "selectionToken":selection.selection_token,
                    }), Purpose::Provision)?;
                } else {
                    let mut input = json!({"directory":selection.comfy_root});
                    if !selection.python.is_empty() {
                        input["python"] = json!(selection.python);
                    }
                    self.forward(id, "inspect", input, Purpose::Provision)?;
                }
            }
            JobOutcome::Success(StorageResult::Installed(installed)) => {
                self.installed = Some(installed);
                let id = self.install_id.take().ok_or("Installation request absent")?;
                self.reply(Some(&id), "completed", json!({"ready":true}))?;
            }
            JobOutcome::Success(StorageResult::Completed(candidate)) => {
                self.publish_installation(*candidate)?;
            }
            JobOutcome::Success(StorageResult::Opened(opened)) => {
                self.opened_installation(opened)?
            }
            JobOutcome::Success(StorageResult::Transferred) => {
                let launch = self.relaunch.as_ref().ok_or("Trial creator absent")?.clone();
                self.submit(StorageWork::Launch, move |context| {
                    launch.launch(&context).map_err(io::Error::other)?;
                    Ok(StorageResult::Launched)
                })?;
            }
            JobOutcome::Success(StorageResult::Launched) => self.launch_completed()?,
            JobOutcome::Failure(cause) => self.storage_failed(cause)?,
            _ => self.storage_failed(io::Error::other(
                "Installer storage operation ended without a result",
            ))?,
        }
        Ok(())
    }

    /// Report a failed operation and retain creator custody through independent recovery.
    fn storage_failed(&mut self, cause: io::Error) -> Result<()> {
        let details = crate::failure_details::Details(&cause).to_string();
        tracing::error!(operation="installer_storage_failure", cause=%details,
            "Installation storage operation failed");
        if let Some(launch) = self.relaunch.clone() {
            self.launch_failure = Some(cause);
            self.retire_launch(launch)
        } else {
            self.report_storage_failure(cause)
        }
    }

    /// Complete Open from the actual creation fact only after creator handles have retired.
    fn launch_completed(&mut self) -> Result<()> {
        self.installed = None;
        self.relaunch = None;
        self.launch_failure = None;
        let id = self.install_id.take();
        if !self.closing && !self.unavailable {
            self.reply(id.as_deref(), "completed", json!({}))?;
        }
        Ok(())
    }

    /// Observe independent cleanup and its retained creation fact through the same typed result.
    pub(super) fn retire_launch(&mut self, launch: ApplicationLaunch) -> Result<()> {
        self.submit(StorageWork::Retire, move |context| {
            launch
                .recover_and_retire(&context)
                .map(StorageResult::Retired)
                .map_err(io::Error::other)
        })
    }

    /// Release the request for retry only after any creator recovery has completed.
    fn report_storage_failure(&mut self, cause: io::Error) -> Result<()> {
        let id = self.inspection_id.take().or_else(|| self.install_id.take());
        if id.is_none() && self.bundle.is_none() {
            self.worker_failed(Box::new(cause))?;
        } else {
            self.reply(
                None,
                "diagnostic",
                json!({"message":crate::failure_details::Details(&cause).to_string()}),
            )?;
            self.reply(
                id.as_deref(),
                "failed",
                json!({"message":super::super::failure::storage_message(&cause, &self.locale)}),
            )?;
        }
        self.prepared = None;
        Ok(())
    }

    /// Complete signed preparation after saved-runtime validation or upstream integration.
    pub(super) fn complete_candidate(
        &mut self,
        prepared: Box<SignedPreparation>,
        runtime: RuntimePaths,
    ) -> Result<()> {
        self.submit(StorageWork::Prepare, move |context| {
            crate::authority_clock::authorize(|now| prepared.complete(&runtime, now, &context))
                .map(|candidate| StorageResult::Completed(Box::new(candidate)))
        })
    }
}
