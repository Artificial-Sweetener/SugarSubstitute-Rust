// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Connect actual transcript revisions to the one startup progress owner.
use super::*;
use sugarsubstitute_application::transcript::TranscriptStore;
use sugarsubstitute_contracts::{lifecycle::*, startup::StartupOperation, transcript::*};

/// Repainting retained output cannot manufacture activity or override frontend readiness.
#[test]
fn output_revisions_advance_one_overall_bar_without_replaying_activity()
-> Result<(), Box<dyn std::error::Error>> {
    let resource = ResourceKey {
        owner: CoordinatorKey {
            namespace: NamespaceId::new([1; 32]),
            epoch: CoordinatorEpoch::new([2; 16])?,
        },
        id: ResourceId::new(1)?,
        generation: ResourceGeneration::new(1)?,
    };
    let mut store = TranscriptStore::new(resource);
    let mut view = StartupView::new(
        Presentation::new("en", RequestedTheme::Dark, false)?,
        sugarsubstitute_ui::accent::AccentPalette::PRODUCT,
    );
    let origin = Instant::now();
    view.sample(StartupPhase::PreparingConnection, Duration::ZERO, origin);
    for (sequence, line, expected) in [
        (1, "Collecting torch\n", StartupOperation::Dependencies),
        (2, "Device: cuda:0\n", StartupOperation::Device),
    ] {
        let elapsed = Duration::from_secs(sequence * 10);
        view.sample(StartupPhase::PreparingConnection, elapsed, origin + elapsed);
        store.apply(&OutputBatch::new(
            resource,
            OutputStream::Stdout,
            StreamSequence::new(sequence)?,
            line.as_bytes(),
            None,
            false,
        )?)?;
        let snapshot = store.snapshot_tail(128)?;
        assert!(view.output(Some(&snapshot)));
        assert_eq!(view.appearance.operation, expected);
        assert!(view.progress().activity.is_some());
        view.sample(
            StartupPhase::PreparingConnection,
            elapsed + Duration::from_secs(3),
            origin + elapsed + Duration::from_secs(3),
        );
        assert!(!view.output(Some(&snapshot)));
        assert_eq!(view.progress().activity, None);
        assert_eq!(view.progress().completed, if sequence == 1 { 0.18 } else { 0.46 });
    }
    view.sample(
        StartupPhase::WebviewLoading,
        Duration::from_secs(30),
        origin + Duration::from_secs(30),
    );
    store.apply(&OutputBatch::new(
        resource,
        OutputStream::Stdout,
        StreamSequence::new(3)?,
        b"Starting server\n",
        None,
        false,
    )?)?;
    assert!(view.output(Some(&store.snapshot_tail(128)?)));
    assert_eq!(view.appearance.operation, StartupOperation::Frontend);
    assert_eq!(view.progress().completed, 0.88);
    Ok(())
}
