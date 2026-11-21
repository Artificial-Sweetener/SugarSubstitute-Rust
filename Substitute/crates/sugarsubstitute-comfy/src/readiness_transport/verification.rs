// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Verify the complete expected installed cohort through a fixed finite request boundary.
use super::{
    probe::{Endpoint, ExpectedAsset},
    types::ReadinessTransportError,
};
use sha2::{Digest, Sha256};
use sugarsubstitute_contracts::configuration::LaunchConfiguration;
pub(super) trait Requests {
    fn get(
        &mut self,
        endpoint: Endpoint<'_>,
        consume: &mut dyn FnMut(&[u8]) -> Result<(), ReadinessTransportError>,
    ) -> Result<(), ReadinessTransportError>;
}
/// Check current metadata against the installed source: a managed backend must serve the
/// installation's exact SugarCubes inventory and run its exact argv; an attached ComfyUI must
/// serve the installed frontend version and its own SugarCubes.
pub(super) fn metadata(
    requests: &mut impl Requests,
    frontend: &str,
    cube: &str,
    paths: &[String],
    managed: Option<&LaunchConfiguration>,
) -> Result<(), ReadinessTransportError> {
    let stats = collect(requests, Endpoint::Stats)?;
    let stats = crate::readiness::decode_system_stats(&stats)
        .map_err(|e| ReadinessTransportError::new("stats_response", e))?;
    let extensions = collect(requests, Endpoint::Extensions)?;
    let extensions = crate::readiness::decode_extensions(&extensions)
        .map_err(|e| ReadinessTransportError::new("extensions_response", e))?;
    let identity = crate::readiness::InstalledFrontendIdentity::new(
        frontend.to_owned(),
        cube.to_owned(),
        paths.to_vec(),
    )
    .map_err(|e| ReadinessTransportError::new("expected_metadata", e))?;
    match managed {
        Some(_) => identity.compare(&stats, &extensions),
        None => identity.compare_attached(&stats, &extensions),
    }
    .map_err(|e| ReadinessTransportError::new("served_metadata", e))?;
    if let Some(configuration) = managed {
        crate::readiness::compare_managed_arguments(&stats, configuration)
            .map_err(|e| ReadinessTransportError::new("served_managed_arguments", e))?;
    }
    Ok(())
}
/// Qualify one complete streamed body before its index can enter retained progress.
pub(super) fn asset(
    requests: &mut impl Requests,
    asset: ExpectedAsset<'_>,
) -> Result<(), ReadinessTransportError> {
    let mut hash = Sha256::new();
    let mut total = 0u64;
    requests.get(Endpoint::Asset(asset), &mut |chunk| {
        total = total
            .checked_add(chunk.len() as u64)
            .filter(|n| *n <= asset.bytes)
            .ok_or_else(|| ReadinessTransportError::invalid("served_asset_length_bound"))?;
        hash.update(chunk);
        Ok(())
    })?;
    let digest: [u8; 32] = hash.finalize().into();
    if total != asset.bytes || digest != asset.hash {
        return Err(ReadinessTransportError::invalid("served_asset_bytes"));
    }
    Ok(())
}
fn collect(
    requests: &mut impl Requests,
    endpoint: Endpoint<'_>,
) -> Result<Vec<u8>, ReadinessTransportError> {
    let bound = endpoint.maximum() as usize;
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(bound)
        .map_err(|cause| ReadinessTransportError::new("served_body_allocation", cause))?;
    if bytes.capacity() != bound {
        return Err(ReadinessTransportError::invalid("served_body_capacity"));
    }
    requests.get(endpoint, &mut |chunk| {
        if bytes.len().checked_add(chunk.len()).is_none_or(|n| n as u64 > endpoint.maximum()) {
            return Err(ReadinessTransportError::invalid("served_body_bound"));
        }
        bytes.extend_from_slice(chunk);
        Ok(())
    })?;
    Ok(bytes)
}
