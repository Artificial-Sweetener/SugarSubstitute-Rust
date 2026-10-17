// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Reject stale native document, source and physical viewport facts before PNG publication.

use super::*;
use std::error::Error;
use sugarsubstitute_contracts::{execution::Generation, lifecycle::*, startup::LaunchId};
type TestResult = Result<(), Box<dyn Error>>;
fn snapshot() -> Result<MainPreviewSnapshot, Box<dyn Error>> {
    Ok(MainPreviewSnapshot {
        document: RenderDocument {
            identity: MainIdentity {
                run: RunKey {
                    owner: CoordinatorKey {
                        namespace: NamespaceId::new([1; 32]),
                        epoch: CoordinatorEpoch::new([2; 16])?,
                    },
                    instance: InstanceId::new(1)?,
                    generation: RunGeneration::new(1)?,
                },
                launch: LaunchId::new(1)?,
                navigation: Generation::new(1)?,
            },
            context: 7,
            frame: DocumentId::new("frame")?,
            loader: DocumentId::new("loader")?,
        },
        native_navigation: 9,
        browser_pid: 11,
        viewport: MainPreviewViewport { left: 0, top: 32, width: 1280, height: 900, dpi: 120 },
        origin: LoopbackOrigin::parse("http://127.0.0.1:8188/", 8188)?,
        versions: [DocumentId::new("1.47.11")?, DocumentId::new("0.15.0")?],
        source_deadline: CounterDeadline::new(100, 10)?,
        issued_qpc: 17,
    })
}

#[test]
fn current_observation_time_keeps_original_cutoff_without_granting_a_new_source() -> TestResult {
    let original = snapshot()?;
    let mut current = original;
    current.issued_qpc = 18;
    assert!(original.validate_current(current).is_ok());
    assert_eq!(original.source_deadline(), CounterDeadline::new(100, 10)?);
    Ok(())
}

#[test]
fn every_changed_document_viewport_or_source_fact_rejects_publication() -> TestResult {
    let original = snapshot()?;
    let mut changes = [original; 14];
    changes[0].document.identity.launch = LaunchId::new(2)?;
    changes[1].document.identity.navigation = Generation::new(2)?;
    changes[2].document.context = 8;
    changes[3].document.frame = DocumentId::new("foreign-frame")?;
    changes[4].document.loader = DocumentId::new("foreign-loader")?;
    changes[5].native_navigation = 10;
    changes[6].browser_pid = 12;
    changes[7].viewport.width += 1;
    changes[8].viewport.top += 1;
    changes[9].viewport.dpi = 96;
    changes[10].origin = LoopbackOrigin::parse("http://127.0.0.1:8189/", 8189)?;
    changes[11].versions[0] = DocumentId::new("1.48.0")?;
    changes[12].versions[1] = DocumentId::new("0.16.0")?;
    changes[13].source_deadline = CounterDeadline::new(101, 10)?;
    for current in changes {
        assert!(original.validate_current(current).is_err());
    }
    Ok(())
}

#[test]
fn changed_viewport_or_dpi_has_closed_native_diagnostic_and_retains_original_cutoff() -> TestResult
{
    use crate::qualification_trace::{NativeFailure, NativePhase};
    let original = snapshot()?;
    let mut observed = [original; 5];
    observed[0].viewport.left += 1;
    observed[1].viewport.top += 1;
    observed[2].viewport.width += 1;
    observed[3].viewport.height += 1;
    observed[4].viewport.dpi += 1;
    for (index, current) in observed.into_iter().enumerate() {
        let mismatch = original.validate_current(current).err().ok_or("Stale viewport admitted")?;
        assert_eq!(
            mismatch,
            if index == 4 { PreviewMismatch::Dpi } else { PreviewMismatch::Viewport }
        );
        let error = mismatch.into_error();
        let detail = NativeFailure::from_error(NativePhase::Revalidate, &error);
        assert_eq!(detail.phase, NativePhase::Revalidate);
        assert_eq!(
            detail.operation,
            if index == 4 { "observe_preview_dpi" } else { "observe_preview_bounds" }
        );
        assert_eq!(detail.hresult, None);
        assert_eq!(
            error.source().and_then(|cause| cause.downcast_ref::<PreviewMismatch>()),
            Some(&mismatch)
        );
        assert_eq!(original, snapshot()?);
        assert_eq!(original.source_deadline(), CounterDeadline::new(100, 10)?);
    }
    Ok(())
}

#[test]
fn changed_source_diagnostic_never_formats_native_document_or_version_payload() -> TestResult {
    use crate::qualification_trace::{NativeFailure, NativePhase};
    let original = snapshot()?;
    let mut observed = [original; 7];
    observed[0].document.frame = DocumentId::new("retained-private-frame")?;
    observed[1].document.loader = DocumentId::new("retained-private-loader")?;
    observed[2].native_navigation += 1;
    observed[3].browser_pid += 1;
    observed[4].origin = LoopbackOrigin::parse("http://127.0.0.1:8189/", 8189)?;
    observed[5].versions[0] = DocumentId::new("retained-private-version")?;
    observed[6].source_deadline = CounterDeadline::new(101, 10)?;
    let expected = [
        PreviewMismatch::Document,
        PreviewMismatch::Document,
        PreviewMismatch::Navigation,
        PreviewMismatch::Browser,
        PreviewMismatch::Origin,
        PreviewMismatch::Versions,
        PreviewMismatch::Deadline,
    ];
    let operations = [
        "observe_preview_content",
        "observe_preview_content",
        "observe_preview_content",
        "observe_preview_browser",
        "observe_preview_content",
        "observe_preview_versions",
        "observe_preview_cutoff",
    ];
    for ((current, expected), operation) in observed.into_iter().zip(expected).zip(operations) {
        let mismatch = original.validate_current(current).err().ok_or("Foreign source admitted")?;
        assert_eq!(mismatch, expected);
        let error = mismatch.into_error();
        let diagnostic = error.to_string();
        assert!(!diagnostic.contains("retained-private"));
        assert!(!diagnostic.contains("127.0.0.1"));
        let detail = NativeFailure::from_error(NativePhase::Revalidate, &error);
        assert_eq!(detail.phase, NativePhase::Revalidate);
        assert_eq!(detail.operation, operation);
        assert_eq!(detail.hresult, None);
        assert_eq!(original, snapshot()?);
    }
    Ok(())
}

#[test]
fn qualification_output_fixture_preserves_exact_basename_bytes_and_hash() -> TestResult {
    let hash = "0123456789abcdef".repeat(4);
    let fixture = MainDownloadFixture::new("0.png".into(), 107745, &hash)?;
    assert_eq!(fixture.name(), "0.png");
    assert_eq!(fixture.bytes(), 107745);
    assert_eq!(fixture.digest_hex(), hash);
    assert!(fixture.allocated_bytes() <= 256);
    Ok(())
}
#[test]
fn qualification_output_fixture_cannot_authorize_native_paths_or_arbitrary_urls() {
    for value in [
        "",
        ".png",
        "../0.png",
        "sub/0.png",
        "sub\\0.png",
        "C:\\0.png",
        "http://127.0.0.1/0.png",
        "0.png:stream",
        ".hidden.png",
        "0.png\0",
        "0.jpg",
    ] {
        assert!(MainDownloadFixture::new(value.into(), 1, &"00".repeat(32)).is_err());
    }
}
#[test]
fn qualification_output_fixture_rejects_bad_hash_length_encoding_and_size() {
    for hash in ["".into(), "00".repeat(31), "gg".repeat(32), "あ".repeat(32)] {
        assert!(MainDownloadFixture::new("0.png".into(), 1, &hash).is_err());
    }
    for bytes in [0, 2 * 1024 * 1024 + 1] {
        assert!(MainDownloadFixture::new("0.png".into(), bytes, &"00".repeat(32)).is_err());
    }
}
