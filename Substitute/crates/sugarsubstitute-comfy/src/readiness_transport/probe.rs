// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Adapt independently retained source facts to fixed finite HTTP verification.
use super::{
    owner::{HttpPort, Transaction, checkpoint},
    types::Listener,
    verification::Requests,
    *,
};
use std::time::Duration;
use sugarsubstitute_contracts::execution::JobContext;

/// Bound one actual phase independently of total installed cohort size.
const ASSETS_PER_STEP: usize = 64;
/// Larger cohorts reserve a full second before issuing another asset body.
const HEADROOM_ASSET_THRESHOLD: usize = 4;
/// One asset and its checked native retirement retain the same original phase headroom.
pub(super) fn asset_headroom(total: usize) -> Duration {
    if total > HEADROOM_ASSET_THRESHOLD { Duration::from_secs(1) } else { Duration::ZERO }
}
#[derive(Clone, Copy)]
pub(super) struct ExpectedAsset<'a> {
    pub(super) path: &'a str,
    pub(super) bytes: u64,
    pub(super) hash: [u8; 32],
}
#[derive(Clone, Copy)]
pub(super) enum Endpoint<'a> {
    Stats,
    Extensions,
    Asset(ExpectedAsset<'a>),
}
impl<'a> Endpoint<'a> {
    pub(super) fn path(self) -> &'a str {
        match self {
            Self::Stats => "/system_stats",
            Self::Extensions => "/extensions",
            Self::Asset(asset) => asset.path,
        }
    }
    pub(super) fn maximum(self) -> u64 {
        match self {
            Self::Stats => crate::readiness::MAX_SYSTEM_STATS_BYTES as u64,
            Self::Extensions => crate::readiness::MAX_EXTENSIONS_BYTES as u64,
            Self::Asset(asset) => asset.bytes,
        }
    }
    pub(super) fn json(self) -> bool {
        matches!(self, Self::Stats | Self::Extensions)
    }
}
struct BoundRequests<'a> {
    port: &'a dyn HttpPort,
    state: &'a mut Transaction,
    input: &'a ReadinessProbeInput,
    context: &'a JobContext,
}
impl Requests for BoundRequests<'_> {
    fn get(
        &mut self,
        endpoint: Endpoint<'_>,
        consume: &mut dyn FnMut(&[u8]) -> Result<(), ReadinessTransportError>,
    ) -> Result<(), ReadinessTransportError> {
        checkpoint(self.context)?;
        self.port.get(self.state, self.input, endpoint, self.context, consume)
    }
}
pub(super) fn inspect(
    port: &dyn HttpPort,
    state: &mut Transaction,
    input: &ReadinessProbeInput,
    context: &JobContext,
) -> Result<(usize, bool), ReadinessTransportError> {
    input.source.validate_live().map_err(|e| ReadinessTransportError::new("expected_source", e))?;
    let manifest = input.source.manifest();
    let start = state.verified_assets;
    let total = manifest.assets().len();
    if total == 0 || start > total {
        return Err(ReadinessTransportError::invalid("served_empty_or_changed_cohort"));
    }
    let mut requests = BoundRequests { port, state, input, context };
    // The source is one immutable pinned cohort. Initial metadata establishes its
    // expected service; final metadata confirms it after every asset completion.
    // Intermediate phases retain that owner and still qualify every HTTP peer.
    if start == 0 || start == total {
        metadata(&mut requests, input)?;
    }
    let mut next = start;
    for asset in manifest.assets().iter().skip(start).take(ASSETS_PER_STEP) {
        checkpoint(context)?;
        let now = context.now();
        let until = context
            .deadline()
            .ok_or_else(|| ReadinessTransportError::invalid("attempt_deadline"))?;
        if !until
            .checked_duration_since(now)
            .is_ok_and(|remaining| remaining > asset_headroom(total))
        {
            break;
        }
        super::verification::asset(
            &mut requests,
            ExpectedAsset { path: asset.path(), bytes: asset.bytes(), hash: asset.sha256() },
        )?;
        next += 1;
    }
    if start == 0 && next == total {
        metadata(&mut requests, input)?;
    }
    input
        .source
        .validate_live()
        .map_err(|e| ReadinessTransportError::new("expected_source_after_requests", e))?;
    checkpoint(context)?;
    // A multi-phase cohort confirms metadata again in its final phase after all
    // asset indices have been committed by their actual timely completions.
    Ok((next, next == total && (start == 0 || start == total)))
}

/// Verify current service metadata against this identical pinned expected cohort.
fn metadata(
    requests: &mut BoundRequests<'_>,
    input: &ReadinessProbeInput,
) -> Result<(), ReadinessTransportError> {
    let manifest = input.source.manifest();
    let configuration = match &input.listener {
        Listener::Managed(_, configuration) => Some(configuration),
        Listener::Attached(_, _) => None,
    };
    super::verification::metadata(
        requests,
        manifest.frontend_version(),
        manifest.cube_version(),
        manifest.sugarcubes_paths(),
        configuration,
    )
}
