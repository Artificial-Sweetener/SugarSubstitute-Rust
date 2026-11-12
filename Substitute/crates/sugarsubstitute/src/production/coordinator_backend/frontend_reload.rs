// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Borrow retained backend authority for independent current-source reload verification.
use super::*;
impl CoordinatorBackend {
    /// Maximum retained effective-target allocation inside the Parent actor's existing credit.
    pub(crate) const FRONTEND_RELOAD_TARGET_BYTES: u64 = 512 * 1024;
    /// Create an independent source inspector without changing the backend's original Ready proof.
    /// The authenticated caller separately admits ResourceCommand::Reload for its exact live lease.
    pub(crate) fn frontend_reload_source(
        &self,
        key: ResourceKey,
    ) -> Result<(InstalledFrontendOwner, BackendTarget)> {
        let record = self.record(key)?;
        record.readiness.as_ref().ok_or(LifecycleRejection::WrongPhase)?.validate_current()?;
        if record.shutdown.is_some() || record.managed_exit.is_some() {
            return Err(LifecycleRejection::WrongPhase.into());
        }
        let target = match &record.target {
            BackendTarget::Managed(_) => {
                let effective = record.resolved.as_ref().ok_or(LifecycleRejection::WrongPhase)?;
                if effective
                    .allocated_bytes()
                    .is_none_or(|bytes| bytes as u64 > Self::FRONTEND_RELOAD_TARGET_BYTES)
                {
                    return Err(LifecycleRejection::Overloaded.into());
                }
                BackendTarget::Managed(effective.clone())
            }
            BackendTarget::Attached(origin) => BackendTarget::Attached(*origin),
        };
        Ok((record.source.fresh_snapshot(), target))
    }
    /// Borrow the actual current backend/listener authority for a freshly inspected source.
    /// This changes no backend lease, process, output stream or another client's source snapshot.
    pub(crate) fn frontend_reload_probe(
        &self,
        key: ResourceKey,
        source: InstalledFrontendLease,
        resources: &BackendResources<'_>,
    ) -> Result<ReadinessProbeInput> {
        let record = self.record(key)?;
        record.readiness.as_ref().ok_or(LifecycleRejection::WrongPhase)?.validate_current()?;
        if record.shutdown.is_some() || record.managed_exit.is_some() {
            return Err(LifecycleRejection::WrongPhase.into());
        }
        Ok(match &record.target {
            BackendTarget::Managed(_) => ReadinessProbeInput::managed(
                source,
                resources
                    .authority
                    .registry()
                    .ok_or(LifecycleRejection::WrongPhase)?
                    .managed_listener_lease(
                        record.launch.ok_or(LifecycleRejection::WrongPhase)?,
                        key,
                    )?,
                record.resolved.as_ref().ok_or(LifecycleRejection::WrongPhase)?.clone(),
            )?,
            BackendTarget::Attached(origin) => {
                ReadinessProbeInput::attached(source, origin.port().get())?
            }
        })
    }
}
