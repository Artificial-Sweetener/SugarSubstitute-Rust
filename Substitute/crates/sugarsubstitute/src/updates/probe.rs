// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Run one explicitly requested engine probe through the existing bounded update-process lifetime.
use std::{error::Error, io, sync::Arc};
use sugarsubstitute_supervision::setup_storage::{
    InheritedUpdaterProbe, RESERVATION_BYTES, TRUST_OPERATION_RESERVATION_BYTES,
    TRUST_RESERVATION_BYTES, TrustStore,
};

/// Fund trust decoding, the complete payload and overlapping protected-storage operations.
pub(super) const PROBE_RESERVATION_BYTES: u64 =
    TRUST_RESERVATION_BYTES + RESERVATION_BYTES + TRUST_OPERATION_RESERVATION_BYTES;

/// Native entry passes only local correlation; repository authority remains compiled into the image.
pub(super) fn run(probe: Arc<InheritedUpdaterProbe>) -> Result<(), Box<dyn Error>> {
    let retained = Arc::clone(&probe);
    let executable = std::env::current_exe()?;
    let mut session = super::session::Session::new(
        PROBE_RESERVATION_BYTES,
        Box::new(move |context| {
            let (repository, _) = super::Distribution::decode(super::EMBEDDED)?.into_parts();
            let trust = TrustStore::open(probe.installation(), repository, context)
                .map_err(io::Error::other)?;
            probe.verify(&trust, &executable, context).map_err(io::Error::other)?;
            Ok(None)
        }),
    );
    sugarsubstitute_desktop::run_headless(&mut session)?;
    session.require_initialization_success()?;
    drop(retained);
    tracing::info!(
        operation = "updater_probe_retired",
        "Updater probe verification and execution retired"
    );
    Ok(())
}
