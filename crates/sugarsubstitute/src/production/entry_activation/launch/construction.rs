// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Construct retained common launch state before any actual native admission.
use super::super::recovery_supervisor::start::RecoveryStart;
use super::*;
use sugarsubstitute_execution::windows_capture_attach::CaptureAttachLaunchPin;
impl DesktopStart {
    /// Bind only actual projected source and native attachment pin to current authority.
    /// No key, creator identity or native source can be reconstructed from observations.
    pub(in crate::production::entry_activation) fn new(
        identity: sugarsubstitute_contracts::startup::MainIdentity,
        qualification: crate::production::qualification::QualificationIntent,
        connection: ConnectionId,
        mut projection: JobCompletion<ProjectedEntry, ProjectionFailure>,
        pin: CaptureAttachLaunchPin,
        resources: &mut EntryResources<'_>,
    ) -> Result<Self> {
        let run = identity.run;
        if resources.authority.key() != Some(run.owner) || !resources.authority.serving() {
            return Err("Actual coordinator authority unavailable".into());
        }
        let value = match &mut projection.outcome {
            JobOutcome::Success(value) => value,
            _ => return Err("Actual entry projection did not succeed".into()),
        };
        let now = resources.client.now()?;
        if now >= value.until {
            return Err("Original entry deadline expired".into());
        }
        let registry =
            resources.authority.registry().ok_or("Actual process registry unavailable")?;
        let profile = WindowsProfileLease::new(run, value.requested_profile.clone(), registry)?;
        let snapshot =
            value.snapshot.take().ok_or("Original startup snapshot already transferred")?;
        Ok(Self::initial(
            identity,
            qualification,
            connection,
            projection,
            profile,
            Some(CapturedLifecycleStart::new(pin, snapshot)),
            None,
        ))
    }
    /// Compose common profile/projection/direct channel state before any native admission.
    pub(in crate::production::entry_activation) fn initial(
        identity: sugarsubstitute_contracts::startup::MainIdentity,
        qualification: crate::production::qualification::QualificationIntent,
        connection: ConnectionId,
        projection: JobCompletion<ProjectedEntry, ProjectionFailure>,
        profile: WindowsProfileLease,
        captured: Option<CapturedLifecycleStart>,
        recovery: Option<RecoveryStart>,
    ) -> Self {
        let sugarsubstitute_contracts::startup::MainIdentity { run, launch, navigation } = identity;
        Self {
            qualification,
            run,
            launch,
            navigation,
            connection,
            projection,
            recovery,
            profile,
            pipe: None,
            output: None,
            pipe_terminal: None,
            checked_retirement: None,
            captured,
            bootstrap: None,
            enrollment: None,
            prepared: None,
            ticket: None,
            original: None,
            acquired: false,
            publication_issued: false,
            custody_watch: None,
            attempted: false,
        }
    }
}
