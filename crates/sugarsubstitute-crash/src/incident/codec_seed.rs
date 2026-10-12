// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Preserve seed identity, bounded build spelling and injected UTC without native authority.
use super::{IncidentStoreError, codec_identity::SubjectRecord};
use serde::{Deserialize, Serialize};
use sugarsubstitute_contracts::incident::{
    BuildIdentity, IncidentRevision, IncidentSeed, IncidentStage,
};

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct SeedRecord {
    key: SubjectRecord,
    revision: u64,
    build: String,
    created: u64,
    stage: StageRecord,
}
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum StageRecord {
    SupervisorInitialization,
    DesktopInitialization,
    Startup,
    Running,
    Shutdown,
}
impl SeedRecord {
    pub(super) fn into_seed(self) -> Result<IncidentSeed, IncidentStoreError> {
        IncidentSeed::new(
            self.key.into_key()?,
            IncidentRevision::new(self.revision)?,
            BuildIdentity::new(&self.build)?,
            super::codec::utc(self.created)?,
            match self.stage {
                StageRecord::SupervisorInitialization => IncidentStage::SupervisorInitialization,
                StageRecord::DesktopInitialization => IncidentStage::DesktopInitialization,
                StageRecord::Startup => IncidentStage::Startup,
                StageRecord::Running => IncidentStage::Running,
                StageRecord::Shutdown => IncidentStage::Shutdown,
            },
        )
        .map_err(Into::into)
    }
}
impl From<&IncidentSeed> for SeedRecord {
    fn from(seed: &IncidentSeed) -> Self {
        Self {
            key: seed.key().into(),
            revision: seed.revision().get(),
            build: seed.build().as_str().into(),
            created: seed.created().unix_seconds(),
            stage: match seed.stage() {
                IncidentStage::SupervisorInitialization => StageRecord::SupervisorInitialization,
                IncidentStage::DesktopInitialization => StageRecord::DesktopInitialization,
                IncidentStage::Startup => StageRecord::Startup,
                IncidentStage::Running => StageRecord::Running,
                IncidentStage::Shutdown => StageRecord::Shutdown,
            },
        }
    }
}
