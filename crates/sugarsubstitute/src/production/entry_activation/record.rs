// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Construct one inert retained run; native capabilities are transferred by their owners.
use super::*;
pub(super) struct Record {
    pub(super) qualification: crate::production::qualification::QualificationIntent,
    pub(super) run: RunKey,
    pub(super) launch: LaunchId,
    pub(super) navigation: Generation,
    pub(super) connection: ConnectionId,
    pub(super) projected: Option<JobCompletion<ProjectedEntry, ProjectionFailure>>,
    pub(super) attach: Option<OwnedJob<CaptureAttachLaunchPin, CaptureAttachFailure>>,
    pub(super) attached_memory: Option<MemoryLease>,
    pub(super) attach_failure: Option<JobCompletion<CaptureAttachLaunchPin, CaptureAttachFailure>>,
    pub(super) released_attach:
        Option<JobCompletion<CaptureAttachRetirement, CaptureAttachFailure>>,
    pub(super) attach_retirement:
        Option<std::result::Result<CaptureAttachRetirementReport, CaptureAttachFailure>>,
    pub(super) start: Option<DesktopStart>,
    pub(super) profile_predecessor:
        Option<sugarsubstitute_supervision::profiles::ProfileRetirement>,
    pub(super) parent: Option<Box<ParentLifecycle>>,
    pub(super) retained_causes: Option<crate::production::parent_lifecycle::causes::BackendCauses>,
    pub(super) preparation: Option<BackendPreparation>,
    pub(super) retired: Option<retirement::RetiredDesktop>,
    pub(super) completed_normal: Option<normal_retirement::CompletedNormal>,
    pub(super) forwarded: Option<forwarding::Forwarded>,
    pub(super) ui_suspected: bool,
    pub(super) replied: bool,
    pub(super) closed: bool,
    pub(super) seed: incident_seed::SeedPreparation,
    pub(super) seeded: bool,
    pub(super) collection: incident_collection::IncidentCollection,
    pub(super) failed: Option<sugarsubstitute_application::supervision::FailedDesktopFact>,
    pub(super) failed_cause: Option<Box<dyn Error>>,
    pub(super) failed_shutdown: Option<ShutdownBudget>,
    pub(super) failed_family_adopted: bool,
    pub(super) pending_desktop_failure: Option<incident_reporting::PendingDesktopFailure>,
    pub(super) restart: restart::RestartState,
}
pub(super) struct PendingProjection {
    pub(super) source: Option<super::source_preparation::Preparation>,
    pub(super) qualification: crate::production::qualification::QualificationIntent,
    pub(super) run: RunKey,
    pub(super) launch: LaunchId,
    pub(super) navigation: Generation,
    pub(super) connection: ConnectionId,
    pub(super) job: Option<OwnedJob<ProjectedEntry, ProjectionFailure>>,
    pub(super) completion: Option<JobCompletion<ProjectedEntry, ProjectionFailure>>,
    pub(super) restart: bool,
}
pub(super) struct RetainedRecord {
    pub(super) record: Box<Record>,
    pub(super) _storage: MemoryClaim,
}
impl PendingProjection {
    /// Failed source-only preparation retains its original completion until activation disposal.
    /// Its observed terminal and returned frame loan permit shutdown without losing that cause.
    pub(super) fn source_terminal(&self) -> bool {
        self.job.is_none()
            && self.completion.is_none()
            && self.source.as_ref().is_some_and(super::source_preparation::Preparation::settled)
    }
}

impl Record {
    /// Initialize the same bounded run state for original and genuinely admitted recovery starts.
    pub(super) fn new(
        identity: sugarsubstitute_contracts::startup::MainIdentity,
        connection: ConnectionId,
        qualification: crate::production::qualification::QualificationIntent,
        replied: bool,
        restart: bool,
    ) -> Self {
        Self {
            qualification,
            run: identity.run,
            launch: identity.launch,
            navigation: identity.navigation,
            connection,
            projected: None,
            attach: None,
            attached_memory: None,
            attach_failure: None,
            released_attach: None,
            attach_retirement: None,
            start: None,
            profile_predecessor: None,
            parent: None,
            retained_causes: None,
            preparation: None,
            retired: None,
            completed_normal: None,
            forwarded: None,
            ui_suspected: false,
            replied,
            closed: false,
            seed: incident_seed::SeedPreparation::new(),
            seeded: false,
            collection: incident_collection::IncidentCollection::new(),
            failed: None,
            failed_cause: None,
            failed_shutdown: None,
            failed_family_adopted: false,
            pending_desktop_failure: None,
            restart: restart::RestartState::for_projection(restart),
        }
    }
}
