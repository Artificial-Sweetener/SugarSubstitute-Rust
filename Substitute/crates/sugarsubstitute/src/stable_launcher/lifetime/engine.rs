// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain receiving engine identity outside job results until launcher retirement.
use super::*;
use std::sync::Mutex;

/// Only worker jobs access the mutex; the event loop owns this handle without waiting on it.
pub(super) struct Guard {
    root: PathBuf,
    executable: PathBuf,
    repository: [u8; 32],
    custody: Mutex<Custody>,
}
/// Trust and image have separately admitted reservations; pinned files never retain trust's peak.
enum Custody {
    Empty,
    Trust(Box<setup_storage::TrustStore>),
    Image { _image: setup_storage::PreparationImage },
    Closed,
}
impl Guard {
    /// Allocate inert receiver state; no filesystem work occurs before execution admission.
    pub(super) fn new(root: PathBuf, executable: PathBuf, repository: [u8; 32]) -> Arc<Self> {
        Arc::new(Self { root, executable, repository, custody: Mutex::new(Custody::Empty) })
    }

    /// Load trust under its own admission before image verification can retain any file pins.
    pub(super) fn open(&self, context: &JobContext) -> io::Result<()> {
        let mut slot =
            self.custody.lock().map_err(|_| io::Error::other("Engine custody is poisoned"))?;
        if !matches!(*slot, Custody::Empty) {
            return Err(io::Error::other("Engine trust was already opened or retired"));
        }
        let trust = setup_storage::TrustStore::open(&self.root, self.repository, context)
            .map_err(io::Error::other)?;
        *slot = Custody::Trust(Box::new(trust));
        Ok(())
    }

    /// Authenticate the actual running image before any candidate can be activated.
    pub(super) fn verify(&self, context: &JobContext) -> io::Result<()> {
        let mut slot =
            self.custody.lock().map_err(|_| io::Error::other("Engine custody is poisoned"))?;
        let Custody::Trust(trust) = &*slot else {
            return Err(io::Error::other("Engine trust is absent or already consumed"));
        };
        let image =
            trust.retain_running_updater(&self.executable, context).map_err(io::Error::other)?;
        image.require_activation()?;
        *slot = Custody::Image { _image: image };
        Ok(())
    }

    /// Call only after observing the verification job's terminal result, on a cleanup worker.
    pub(super) fn retire(&self) {
        let mut slot = self.custody.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        *slot = Custody::Closed;
    }
}

impl Launcher {
    /// This role may activate locally cached work, but cannot delegate to another engine.
    pub(in crate::stable_launcher) fn activation(
        source: Arc<sugarsubstitute_supervision::application_launch::broker::ActivationSender>,
        executable: PathBuf,
        repository: [u8; 32],
    ) -> Self {
        let root = source.installation().to_owned();
        let mut launcher = Self::new(root.clone());
        launcher.source = Some(source);
        launcher.engine = Some(Guard::new(root, executable, repository));
        launcher.choice = Choice::OpenEngine;
        launcher
    }
}
