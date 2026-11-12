// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Publish earlier installations or repair the currently accepted identical distribution.
use super::*;
use crate::setup_storage::{InstallationLaunch, InstallationRun, SelectionStore};
use sugarsubstitute_contracts::installation::StartupSelection;

impl PreparedInstallation {
    /// Publish an embedded distribution or restore shell access to its already accepted copy.
    /// Selection-enabled replacements require a health-checked trial before publication.
    /// # Errors
    /// Preserves the current selection and shortcut on an unaccepted replacement, unresolved
    /// trial, damaged accepted payload, competing run, downloaded package or I/O failure.
    pub fn activate(
        &self,
        runtime: &RuntimePaths,
        context: &JobContext,
    ) -> io::Result<InstalledApplication> {
        check(context)?;
        if self.bundle.download_identity().is_some() {
            return Err(invalid("Downloaded updates require current installation authorization"));
        }
        if let Some(application) = self.repair_accepted(runtime, context)? {
            return Ok(application);
        }
        self.complete(runtime, context)?;
        check(context)?;
        super::super::entrypoint::publish(&self.session.admission.root, &self.generation, context)?;
        Ok(InstalledApplication {
            executable: self.generation.join("sugarsubstitute.exe"),
            directory: self.generation.clone(),
        })
    }

    /// Reuse existing health authority only for the exact accepted manifest and verified files.
    /// The live run and payload guards remain held through atomic shortcut publication.
    fn repair_accepted(
        &self,
        runtime: &RuntimePaths,
        context: &JobContext,
    ) -> io::Result<Option<InstalledApplication>> {
        let Some(store) = SelectionStore::existing(self.session.admission.root.path(), context)
            .map_err(io::Error::other)?
        else {
            return Ok(None);
        };
        let snapshot = store
            .load(context)
            .map_err(io::Error::other)?
            .ok_or_else(|| invalid("Installation selection is missing; repair is required"))?;
        let StartupSelection::Accepted(generation) = snapshot.startup_selection() else {
            return Err(invalid(
                "Installation activation has not finished; the existing shortcut has been preserved",
            ));
        };
        if generation.package.as_str() != self.bundle.digest() {
            return Err(invalid(
                "The replacement application must pass its startup check before the shortcut can change",
            ));
        }
        if self.maintenance_runtime() != Some(runtime) {
            return Err(invalid("Maintenance runtime differs from the saved installation"));
        }
        let mut run =
            InstallationRun::acquire(Arc::new(store), context).map_err(io::Error::other)?;
        let InstallationLaunch::Application(application) =
            run.select_launch(None, context).map_err(io::Error::other)?
        else {
            return Err(invalid("Installation selection changed before shortcut repair"));
        };
        if application.generation() != generation {
            return Err(invalid("Accepted application changed before shortcut repair"));
        }
        application
            .repair_under_admission(&self.session.admission, context)
            .map(Some)
            .map_err(io::Error::other)
    }
}
