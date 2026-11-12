// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain installation mutation custody before package delivery and through preparation.
use super::*;

/// One native installation and its saved choices, acquired independently of package delivery.
/// Retain this owner while authenticating or receiving a package. It owns setup exclusion only;
/// an already running application may keep its separate run and resource leases.
/// Consuming the session transfers the same exclusion into preparation or candidate recovery.
pub struct InstallationSession {
    pub(super) selection: Selection,
    pub(super) admission: Admission,
    pub(super) existing: Option<ExistingInstallation>,
}
impl InstallationSession {
    /// Acquire a fresh installation using validated choices, without reserving a generation.
    /// Explicit retry resumes protected staging whose first configuration is incomplete,
    /// preserving unfinished predecessors. Completed/unknown generations require recovery.
    /// # Errors
    /// Rejects unknown populated directories, existing installations and competing setup owners.
    /// Preserves all filesystem state on failure; OS exclusion releases when this owner drops.
    pub fn create(selection: Selection, context: &JobContext) -> io::Result<Self> {
        check(context)?;
        selection.validate()?;
        let admission = Admission::open(&selection.install_root, true, context)?;
        if ExistingInstallation::read(&admission.root, context)?.is_some() {
            return Err(invalid("Use update or repair for this existing installation"));
        }
        check(context)?;
        Ok(Self { selection, admission, existing: None })
    }

    /// Acquire update or repair custody and pin saved native configuration before delivery.
    /// ComfyUI paths are preserved as data; its interpreter and application are not started.
    /// # Errors
    /// Rejects unknown installations, damaged saved state and competing setup owners.
    pub fn maintain(root: &Path, context: &JobContext) -> io::Result<Self> {
        check(context)?;
        let admission = Admission::open(root, false, context)?;
        let existing = ExistingInstallation::read(&admission.root, context)?
            .ok_or_else(|| invalid("No selected native installation is available to update"))?;
        let selection = existing.summary.selection.clone();
        check(context)?;
        Ok(Self { selection, admission, existing: Some(existing) })
    }

    /// Pinned installation root for trust and package-cache operations on admitted workers.
    /// The path alone does not transfer this session's mutation authority.
    pub fn directory(&self) -> &Path {
        self.admission.root.path()
    }

    /// Create the protected user folders every installed host expects before its first start:
    /// the user root, the desktop host's data and crash-capture storage.
    ///
    /// Only protected creation gives these folders their account-only access, and the host
    /// verifies that protection instead of creating them, so setup provisions them for every
    /// preparation. Existing folders keep their contents and are only verified.
    /// # Errors
    /// Preserves native creation, protection and cancellation failures without partial cleanup.
    pub(super) fn provision_user_folders(&self, context: &JobContext) -> io::Result<()> {
        let layout =
            sugarsubstitute_configuration::layout::InstallationLayout::new(self.directory());
        crate::durable_storage::protection::with(context, |protection| {
            for folder in [
                layout.user(),
                layout.user().join(sugarsubstitute_configuration::layout::DESKTOP_DATA),
                layout.capture_root(),
            ] {
                protection.directory(
                    &folder,
                    crate::durable_storage::protected::directories::Mode::OpenOrCreate,
                    context,
                )?;
            }
            Ok(())
        })
    }

    /// Reserve a protected generation and transfer this session into package preparation.
    /// Package verification and delivery may happen while this session retains the installation;
    /// this transition never releases and reacquires its mutation lock.
    /// # Errors
    /// Preserves existing data and partial generations on cancellation or native failure.
    pub fn prepare(
        self,
        bundle: Arc<VerifiedBundle>,
        context: &JobContext,
    ) -> io::Result<PreparedInstallation> {
        check(context)?;
        let root = &self.admission.root;
        let releases = Directory::open(
            &root.path().join(sugarsubstitute_configuration::layout::RELEASES),
            true,
        )?;
        let generation =
            releases.path().join(crate::setup_storage::identity::generation()?.as_str());
        self.provision_user_folders(context)?;
        let (pinned, marker) = generation::create(&generation, bundle.digest(), context)?;
        Ok(PreparedInstallation {
            session: self,
            generation,
            bundle,
            _generation: pinned,
            _generation_marker: marker,
        })
    }
}

#[cfg(test)]
mod tests;
