// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Supply independent expected-file and fixed HTTP boundary facts to full verification.
use super::*;
use crate::readiness_transport::{
    probe::ExpectedAsset,
    types::ReadinessTransportError,
    verification::{Requests, asset as verify_asset, metadata},
};
use serde_json::json;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
struct Boundary {
    responses: BTreeMap<String, Vec<u8>>,
    calls: Vec<String>,
    split: usize,
    fail: Option<usize>,
}
impl Requests for Boundary {
    fn get(
        &mut self,
        endpoint: Endpoint<'_>,
        consume: &mut dyn FnMut(&[u8]) -> Result<(), ReadinessTransportError>,
    ) -> Result<(), ReadinessTransportError> {
        self.calls.push(endpoint.path().into());
        if self.fail == Some(self.calls.len()) {
            return Err(ReadinessTransportError::new(
                "fake_http_phase",
                std::io::Error::other("original-request-cause"),
            ));
        }
        let bytes = self
            .responses
            .get(endpoint.path())
            .ok_or_else(|| ReadinessTransportError::invalid("missing_fixture_response"))?;
        for chunk in bytes.chunks(self.split) {
            consume(chunk)?;
        }
        Ok(())
    }
}
fn stats(argv: &[&str]) -> Result<Vec<u8>, serde_json::Error> {
    serde_json::to_vec(
        &json!({"system":{"os":"win32","ram_total":68719476736u64,"ram_free":40000000000u64,
        "comfyui_version":"0.29.0","required_frontend_version":"1.47.11",
        "installed_templates_version":"1.0.0","required_templates_version":"1.0.0",
        "comfy_package_versions":[{"name":"comfyui-frontend-package","installed":"1.47.11","required":"1.47.11"}],
        "python_version":"3.12.0","pytorch_version":"2.9.0","embedded_python":false,
        "deploy_environment":"local","argv":argv},"devices":[]}),
    )
}
fn boundary() -> Result<Boundary, serde_json::Error> {
    Ok(Boundary {
        responses: BTreeMap::from([
            ("/system_stats".into(), stats(&["unowned.py"])?),
            ("/extensions".into(), br#"["/extensions/SugarCubes/main.js"]"#.to_vec()),
            ("/index.html".into(), b"expected actual file bytes".to_vec()),
        ]),
        calls: Vec::new(),
        split: 7,
        fail: None,
    })
}
fn expected() -> ExpectedAsset<'static> {
    ExpectedAsset {
        path: "/index.html",
        bytes: 26,
        hash: Sha256::digest(b"expected actual file bytes").into(),
    }
}
fn inspect(
    boundary: &mut Boundary,
    asset: ExpectedAsset<'_>,
) -> Result<(), ReadinessTransportError> {
    metadata(boundary, "1.47.11", "0.15.0", &["/extensions/SugarCubes/main.js".into()], None)?;
    verify_asset(boundary, asset)
}
#[test]
fn complete_metadata_and_exact_streamed_file_hash_pass_across_chunk_boundaries() -> TestResult {
    let mut boundary = boundary()?;
    let mut asset = expected();
    asset.bytes = boundary.responses["/index.html"].len() as u64;
    inspect(&mut boundary, asset)?;
    assert_eq!(boundary.calls, ["/system_stats", "/extensions", "/index.html"]);
    Ok(())
}
#[test]
fn reported_version_cannot_mask_wrong_served_asset_bytes() -> TestResult {
    let mut boundary = boundary()?;
    boundary.responses.insert("/index.html".into(), b"different actual file bytes".to_vec());
    assert!(inspect(&mut boundary, expected()).is_err());
    assert_eq!(boundary.calls.len(), 3);
    Ok(())
}
#[test]
fn omitted_cube_inventory_fails_before_any_asset_request() -> TestResult {
    let mut boundary = boundary()?;
    boundary.responses.insert("/extensions".into(), b"[]".to_vec());
    assert!(inspect(&mut boundary, expected()).is_err());
    assert_eq!(boundary.calls.len(), 2);
    Ok(())
}
#[test]
fn oversized_and_malformed_complete_metadata_fail_at_their_endpoint() -> TestResult {
    let mut boundary = boundary()?;
    boundary
        .responses
        .insert("/system_stats".into(), vec![b' '; crate::readiness::MAX_SYSTEM_STATS_BYTES + 1]);
    assert!(inspect(&mut boundary, expected()).is_err());
    assert_eq!(boundary.calls.len(), 1);
    boundary.responses.insert("/system_stats".into(), b"{} trailing".to_vec());
    boundary.calls.clear();
    assert!(inspect(&mut boundary, expected()).is_err());
    assert_eq!(boundary.calls.len(), 1);
    Ok(())
}
#[test]
fn cancellation_or_failure_at_every_request_stops_without_internal_retry() -> TestResult {
    use std::error::Error;
    for phase in 1..=3 {
        let mut boundary = boundary()?;
        boundary.fail = Some(phase);
        let error = inspect(&mut boundary, expected()).err().ok_or("Expected phase failure")?;
        assert_eq!(error.stage(), "fake_http_phase");
        assert_eq!(
            error.source().ok_or("Missing original cause")?.to_string(),
            "original-request-cause"
        );
        assert_eq!(boundary.calls.len(), phase);
    }
    Ok(())
}
#[test]
fn truncated_and_excess_asset_bodies_do_not_match_expected_source() -> TestResult {
    for body in [b"expected actual file byte".as_slice(), b"expected actual file bytes plus"] {
        let mut boundary = boundary()?;
        boundary.responses.insert("/index.html".into(), body.to_vec());
        assert!(inspect(&mut boundary, expected()).is_err());
    }
    Ok(())
}
#[test]
fn managed_response_requires_full_effective_argv_while_attachment_does_not() -> TestResult {
    use sugarsubstitute_contracts::configuration::LaunchConfiguration;
    let configuration = LaunchConfiguration::new(
        "E:\\fixture".into(),
        "E:\\fixture\\python.exe".into(),
        vec!["-u".into(), "main.py".into()],
        8188,
    )?;
    let mut boundary = boundary()?;
    assert!(
        metadata(
            &mut boundary,
            "1.47.11",
            "0.15.0",
            &["/extensions/SugarCubes/main.js".into()],
            Some(&configuration)
        )
        .is_err()
    );
    assert_eq!(boundary.calls.len(), 2);
    Ok(())
}
