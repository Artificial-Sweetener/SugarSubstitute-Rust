// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Adapt independently retained source facts to fixed finite HTTP verification.
use super::{
    owner::{HttpPort, Transaction, checkpoint},
    types::Listener,
    verification::Requests,
    *,
};
use sugarsubstitute_contracts::execution::JobContext;
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
) -> Result<(), ReadinessTransportError> {
    input.source.validate_live().map_err(|e| ReadinessTransportError::new("expected_source", e))?;
    let manifest = input.source.manifest();
    let configuration = match &input.listener {
        Listener::Managed(_, configuration) => Some(configuration),
        Listener::Attached(_, _) => None,
    };
    let mut requests = BoundRequests { port, state, input, context };
    super::verification::verify(
        &mut requests,
        manifest.frontend_version(),
        manifest.cube_version(),
        manifest.sugarcubes_paths(),
        manifest.assets().iter().map(|asset| ExpectedAsset {
            path: asset.path(),
            bytes: asset.bytes(),
            hash: asset.sha256(),
        }),
        configuration,
    )?;
    input
        .source
        .validate_live()
        .map_err(|e| ReadinessTransportError::new("expected_source_after_requests", e))?;
    checkpoint(context)
}
