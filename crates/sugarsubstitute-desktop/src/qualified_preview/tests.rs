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
    assert!(original.same_source(current));
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
        assert!(!original.same_source(current));
    }
    Ok(())
}
