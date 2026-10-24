// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Separate protected offline authority from configured network capabilities.
use crate::{
    setup_storage::TrustStore,
    update_repository::{Failure, Repository},
};
use std::{io, path::Path};
use sugarsubstitute_contracts::execution::JobContext;

pub(in crate::update_preparation) enum Source {
    /// This campaign owns no HTTP client or transport policy.
    Installed,
    /// Online discovery remains bound to the producing distribution's repository.
    Distribution { identity: [u8; 32], repository: Repository },
}
impl Source {
    /// Open authority under the selected campaign's exact trust boundary.
    pub(in crate::update_preparation) fn open(
        &self,
        installation: &Path,
        context: &JobContext,
    ) -> Result<TrustStore, Failure> {
        Ok(match self {
            Self::Installed => TrustStore::open_installed(installation, context)?,
            Self::Distribution { identity, .. } => {
                TrustStore::open(installation, *identity, context)?
            }
        })
    }

    /// Offline state cannot transition into a network operation.
    pub(in crate::update_preparation) fn network(&mut self) -> Result<&mut Repository, Failure> {
        match self {
            Self::Distribution { repository, .. } => Ok(repository),
            Self::Installed => Err(io::Error::other("Offline startup cannot access HTTP").into()),
        }
    }

    /// Retirement observes HTTP custody only when the campaign owns that capability.
    pub(in crate::update_preparation) fn retire(&mut self) -> Result<(), Failure> {
        match self {
            Self::Distribution { repository, .. } => Ok(repository.retire()?),
            Self::Installed => Ok(()),
        }
    }

    /// Only a settled transport can release prepared update custody.
    pub(in crate::update_preparation) fn settled(&self) -> bool {
        match self {
            Self::Distribution { repository, .. } => repository.settled(),
            Self::Installed => true,
        }
    }

    /// Select cache-only state transitions without inventing a network endpoint.
    pub(in crate::update_preparation) fn cached(&self) -> bool {
        matches!(self, Self::Installed)
    }
}
