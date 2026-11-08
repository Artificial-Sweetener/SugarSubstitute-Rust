// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Settle source-host data only after authenticated election, without adopting an installation.
mod profile;
use super::*;
use crate::{
    coordinator_entry::EntrySourceDataAdmission,
    setup_storage::{SelectionStore, ownership},
};
use std::path::PathBuf;
use sugarsubstitute_contracts::{
    configuration::ProfileId, installation::InstallationId, lifecycle::RunKey,
};
use sugarsubstitute_execution::AuthenticatedEntryLaunch;

/// Opaque source-host settlement and immutable passive input custody for the same elected run.
/// This owns no installation selection, backend storage, process launch or old diagnostic writer.
pub struct SourceLaunchSettlement {
    admission: EntrySourceDataAdmission,
    host: Option<VerifiedHost>,
    originals: Option<reading::PinnedOriginals>,
    adoption: Option<std::fs::File>,
    profile_directory: Option<PathBuf>,
    correlation: Option<InstallationId>,
    profile: Option<ProfileId>,
    reference: Option<EntryOriginalsReference>,
    legacy: Option<super::super::runtime::LegacySources>,
    memory: Option<MemoryClaim>,
    complete: bool,
}
impl SourceLaunchSettlement {
    /// Retain actual admission before any worker/native effects or fallible parsing.
    pub fn new(admission: EntrySourceDataAdmission) -> Self {
        Self {
            admission,
            host: None,
            originals: None,
            adoption: None,
            profile_directory: None,
            correlation: None,
            profile: None,
            reference: None,
            legacy: None,
            memory: None,
            complete: false,
        }
    }
    /// Resolve the compact reference through this actual running image's private fixed namespace.
    /// The caller reserves32MiB under the same authenticated request's original deadline and
    /// retains this owner even on failure or cancellation. No mutable original path is reopened.
    /// # Errors
    /// Rejects revoked/foreign admission, installed-state ambiguity, changed copies/intent,
    /// reused references, uncertain legacy profile ownership and genuine original controls.
    pub fn settle(
        &mut self,
        image_directory: &Path,
        request: &AuthenticatedEntryLaunch,
        run: RunKey,
        archives: &mut impl FnMut(
            &Path,
            &JobContext,
        ) -> io::Result<super::super::HistoricalArchiveDisposition>,
        context: &JobContext,
    ) -> io::Result<()> {
        self.admission.validate(request, context).map_err(io::Error::other)?;
        if self.memory.is_some() || self.complete {
            return Err(invalid("Source launch settlement already began"));
        }
        if run.owner != self.admission.owner() {
            return Err(invalid("Source settlement run differs from actual elected owner"));
        }
        self.memory = Some(context.memory().claim(8 * 1024 * 1024).map_err(io::Error::other)?);
        self.host = Some(VerifiedHost::capture(image_directory, context)?);
        let host =
            self.host.as_ref().ok_or_else(|| invalid("Actual source host custody absent"))?;
        if host.producer.image_directory != self.admission.image_directory()
            || host.producer.namespace() != run.owner.namespace
        {
            return Err(invalid(
                "Source settlement image or namespace differs from actual election",
            ));
        }
        // Existing selection is an installed concern even when damaged. No source fallback can
        // manufacture its identity or infer managed Comfy ownership from an earlier root marker.
        require_source_host(&host.root, context)?;
        let peer = request.peer();
        if peer.namespace() != run.owner.namespace
            || peer.account_sid() != host.producer.account_sid
            || peer.session_id() != host.producer.session
            || peer.host_image_identity() != host.producer.image
        {
            return Err(invalid("Source snapshot peer differs from actual account/image custody"));
        }
        let mut expected = host.producer.clone();
        expected.process = peer.process_id();
        expected.creation_100ns = peer.creation_time_100ns();
        let reference =
            request.view().map_err(io::Error::other)?.originals().ok_or_else(|| {
                invalid("Source launch requires its authenticated passive originals")
            })?;
        let parent = host.originals_directory(false, context)?;
        self.originals = Some(reading::PinnedOriginals::new(&parent, reference)?);
        let originals =
            self.originals.as_mut().ok_or_else(|| invalid("Passive original custody absent"))?;
        originals.load(reference, &expected, context)?;
        self.reference = Some(reference);
        let manifest =
            originals.manifest.as_ref().ok_or_else(|| invalid("Passive manifest absent"))?;
        let (paths, configuration) =
            manifest.intent(&originals.startup, originals.descriptor.as_deref())?;
        if !manifest::matches_sealed(
            &paths,
            &configuration,
            request.view().map_err(io::Error::other)?,
        ) {
            return Err(invalid(
                "Passive original intent differs from the authenticated sealed request",
            ));
        }
        self.admission.validate(request, context).map_err(io::Error::other)?;
        self.adoption = Some(adoption::bind(&parent, originals, request, run, context)?);
        let layout =
            sugarsubstitute_configuration::layout::InstallationLayout::new(host.root.path());
        let correlation = crate::session_identity::session_host_identity(
            &layout.user(),
            configuration.profile(),
            None,
            context,
        )?;
        self.correlation = Some(correlation);
        self.profile = Some(configuration.profile().clone());
        self.legacy = Some(super::super::runtime::settle_source(
            &host.root,
            crate::durable_storage::imports::ImportAssociation::SourceHost {
                correlation: self
                    .correlation
                    .as_ref()
                    .ok_or_else(|| invalid("Source correlation absent"))?,
                profile: configuration.profile(),
                namespace: run.owner.namespace,
            },
            run.owner.namespace,
            context,
        )?);
        self.profile_directory = Some(profile::settle(
            host,
            &paths,
            self.correlation.as_ref().ok_or_else(|| invalid("Source correlation absent"))?,
            configuration.profile(),
            run.owner.namespace,
            archives,
            context,
        )?);
        self.admission.validate(request, context).map_err(io::Error::other)?;
        self.complete = true;
        Ok(())
    }
    /// Borrow only the completed confined profile selection; partial work never supplies an override.
    pub fn profile_directory(&self) -> Option<&Path> {
        self.complete.then_some(self.profile_directory.as_deref()).flatten()
    }
    /// Borrow the canonical provider's completed source correlation; it is never an installed ID.
    pub fn source_correlation(&self) -> Option<&InstallationId> {
        self.complete.then_some(self.correlation.as_ref()).flatten()
    }
    /// Observe only the complete original capture commitment for projection/restart correlation.
    pub fn originals_reference(&self) -> Option<EntryOriginalsReference> {
        self.complete.then_some(self.reference).flatten()
    }
    /// Release historical source exclusions after the caller observes this successful job terminal.
    /// Immutable passive originals, binding and current host provenance remain retained separately.
    /// # Errors
    /// Incomplete settlement cannot release or expose successful migration custody.
    pub fn observed_settlement_terminal(&mut self) -> io::Result<()> {
        if !self.complete {
            return Err(invalid("Source settlement is not complete"));
        }
        self.legacy = None;
        Ok(())
    }
    /// Open a fresh passive reader only for this completed source-host/profile/namespace scope.
    /// Each later read/export supplies its own finite context; this does not renew startup,
    /// enroll claims or admit an installation/backend writer.
    /// # Errors
    /// Preserves missing/changed receipts, source custody, control and ownership refusals.
    pub fn preserved_diagnostics(
        &self,
        context: &JobContext,
    ) -> io::Result<super::super::PreservedDiagnosticSource> {
        if !self.complete {
            return Err(invalid("Source-host settlement is incomplete"));
        }
        let host = self.host.as_ref().ok_or_else(|| invalid("Source host custody absent"))?;
        let correlation =
            self.correlation.as_ref().ok_or_else(|| invalid("Source correlation absent"))?;
        let profile = self.profile.as_ref().ok_or_else(|| invalid("Source profile absent"))?;
        super::super::PreservedDiagnosticSource::open_at(
            &host.root,
            crate::durable_storage::imports::ImportAssociation::SourceHost {
                correlation,
                profile,
                namespace: self.admission.owner().namespace,
            },
            self.admission.owner().namespace,
            context,
        )
    }
}

/// An existing selection is queried only after the native installation marker owner admitted it.
/// A truly fresh source root has no installation state; damaged namespace evidence still refuses.
pub(super) fn require_source_host(root: &Directory, context: &JobContext) -> io::Result<()> {
    if ownership::running_present(root, false, context)?
        && SelectionStore::existing(root.path(), context).map_err(io::Error::other)?.is_some()
    {
        return Err(invalid("Installed selection cannot become a source-host migration"));
    }
    Ok(())
}
