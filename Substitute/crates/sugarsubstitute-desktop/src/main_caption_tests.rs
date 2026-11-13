// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Caption geometry for every update and message state, with a fixed-width fake font.
use super::*;
use crate::{
    MainBackendState, MainIdentity, MainRecoveryLabels, WorkflowActivity,
    main_backend::Recovery,
    main_updates::{CaptionUpdates, Notice, UpdateFeed, UpdateGlyph},
};
use sugarsubstitute_contracts::{
    execution::Generation,
    lifecycle::{
        ConnectionId, CoordinatorEpoch, CoordinatorKey, InstanceId, LeaseId, LeaseKey, NamespaceId,
        ResourceGeneration, ResourceId, ResourceKey, RunGeneration, RunKey,
    },
    localization::messages,
    startup::LaunchId,
};

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

const CAPTIONS: MainCaptionLabels =
    MainCaptionLabels { minimize: "min", maximize: "max", restore: "restore", close: "close" };
const RECOVERY: MainRecoveryLabels = MainRecoveryLabels { retry: "Retry", retry_ready: "Ready" };

fn identity() -> TestResult<MainIdentity> {
    Ok(MainIdentity {
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
    })
}

/// Six logical pixels per character.
fn width(text: &str) -> f64 {
    text.chars().count() as f64 * 6.0
}

fn measure(text: &str) -> Result<f64, std::convert::Infallible> {
    Ok(width(text))
}

fn layout(width: f64, caption: &CaptionUpdates, retry: bool) -> TestResult<[AccessibleControl; 9]> {
    let current = identity()?;
    let mut recovery = Recovery::new(current);
    if retry {
        let failed = LeaseKey {
            run: current.run,
            resource: ResourceKey {
                owner: current.run.owner,
                id: ResourceId::new(1)?,
                generation: ResourceGeneration::new(1)?,
            },
            connection: ConnectionId::new(1)?,
            id: LeaseId::new(1)?,
        };
        recovery.present(current, failed, MainBackendState::RetryReady)?;
    }
    let view = caption.view(messages("en"));
    Ok(caption_controls(width, false, CAPTIONS, recovery.presentation(RECOVERY), &view, measure)?)
}

fn ends(control: &AccessibleControl) -> f64 {
    control.bounds.x + control.bounds.width
}

#[test]
fn window_buttons_keep_their_reference_geometry() -> TestResult {
    for (width, maximized) in [(304.0, false), (1280.0, false), (1600.0, true)] {
        let view = CaptionUpdates::default().view(messages("en"));
        let retry = Recovery::new(identity()?).presentation(RECOVERY);
        let controls = caption_controls(width, maximized, CAPTIONS, retry, &view, measure)?;
        assert_eq!(controls.map(|control| control.id), [1, 2, 3, 4, 5, 6, 7, 8, 9]);
        for (index, control) in controls[..3].iter().enumerate() {
            assert_eq!(control.bounds.x, width - 138.0 + index as f64 * 46.0);
            assert_eq!(control.bounds.width, 46.0);
            assert_eq!(control.bounds.y, 1.0);
            assert_eq!(control.bounds.height, 32.0);
            assert_eq!(control.close, index == 2);
        }
        assert_eq!(controls[1].label, if maximized { CAPTIONS.restore } else { CAPTIONS.maximize });
        assert_eq!(controls[3].bounds.x, 8.0);
        assert!(ends(&controls[3]) <= controls[0].bounds.x);
        assert!(controls[3..].iter().all(|control| !control.close && !control.visible));
    }
    Ok(())
}

#[test]
fn the_update_button_sits_beside_minimize_and_the_offer_beside_it() -> TestResult {
    let mut caption = CaptionUpdates::default();
    caption.observe(UpdateFeed {
        glyph: UpdateGlyph::Ready,
        saving: false,
        workflow: WorkflowActivity::Idle,
    });
    caption.activate(crate::main_updates::UPDATE);
    let controls = layout(1280.0, &caption, false)?;
    let [minimize, .., update, offer, line, _, _] = &controls;
    assert_eq!((update.bounds.x, update.bounds.width), (1280.0 - 184.0, 46.0));
    assert!(update.visible && offer.visible && !line.visible);
    assert_eq!(ends(offer), update.bounds.x);
    assert_eq!(ends(update), minimize.bounds.x);
    assert_eq!(offer.bounds.width, width("Restart to apply") + 16.0);
    Ok(())
}

#[test]
fn a_message_line_and_its_dismiss_fill_the_left_without_crossing_the_buttons() -> TestResult {
    let mut caption = CaptionUpdates::default();
    caption.observe(UpdateFeed {
        glyph: UpdateGlyph::Warning,
        saving: false,
        workflow: WorkflowActivity::Idle,
    });
    caption.notify(Notice::NotSaved { workspace: true, inputs: true });
    let wide = layout(1600.0, &caption, false)?;
    let text = messages("en").closing.not_saved_both;
    assert!(wide[6].visible && wide[8].visible && !wide[7].visible);
    assert_eq!((wide[6].bounds.x, wide[6].bounds.width), (8.0, width(text) + 16.0));
    assert_eq!(wide[8].bounds.x, ends(&wide[6]));

    // A narrow window shortens the line so the dismiss stays clear of the update button.
    let narrow = layout(560.0, &caption, false)?;
    assert!(narrow[6].visible && narrow[8].visible);
    assert!(narrow[6].bounds.width < width(text));
    assert!(ends(&narrow[8]) <= narrow[4].bounds.x);

    // Too narrow for any words: the line and dismiss hide rather than overlap.
    let tiny = layout(220.0, &caption, false)?;
    assert!(!tiny[6].visible && !tiny[8].visible);
    assert!(tiny.iter().all(|control| control.bounds.width >= 1.0));
    Ok(())
}

#[test]
fn the_backend_retry_takes_the_left_over_a_message() -> TestResult {
    let mut caption = CaptionUpdates::default();
    caption.notify(Notice::NotSaved { workspace: true, inputs: false });
    let controls = layout(1280.0, &caption, true)?;
    assert!(controls[3].visible);
    assert!(!controls[6].visible && !controls[8].visible);
    let controls = layout(1280.0, &caption, false)?;
    assert!(!controls[3].visible && controls[6].visible);
    Ok(())
}
