// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Check the exact public presentation contract, and lay the caption and the page out through
//! their real owners at the scales users run, in physical client pixels.
use super::*;
use crate::{
    AccessibleControl, MainCaptionLabels, MainIdentity, MainRecoveryLabels,
    control_geometry::physical_bounds,
    main_backend::Recovery,
    main_caption::{PEERS, caption_controls, page_top},
    main_updates::{CaptionUpdates, DISMISS, LINE, Notice},
};
use sugarsubstitute_contracts::{
    execution::Generation,
    lifecycle::{CoordinatorEpoch, CoordinatorKey, InstanceId, NamespaceId, RunGeneration, RunKey},
    localization::messages,
    startup::LaunchId,
};
use windows::Win32::Foundation::RECT;

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

/// 100%, 150% and 175%; the proof desktop runs at 168 DPI, which is 175%.
const SCALES: [f64; 3] = [1.0, 1.5, 1.75];
const LABELS: MainCaptionLabels =
    MainCaptionLabels { minimize: "min", maximize: "max", restore: "restore", close: "close" };
const RECOVERY: MainRecoveryLabels = MainRecoveryLabels { retry: "Retry", retry_ready: "Ready" };

#[test]
fn public_desktop_frame_receipt_rejects_unsupported_or_foreign_presentation() {
    for (json, dark, accepted) in [
        (
            r#"{"schemaVersion":1,"captionWidth":138,"titlebarHeight":34,"darkTheme":true,"mounted":true}"#,
            true,
            true,
        ),
        (
            r#"{"schemaVersion":1,"captionWidth":138,"titlebarHeight":34,"darkTheme":false,"mounted":false}"#,
            false,
            true,
        ),
        (
            r#"{"schemaVersion":2,"captionWidth":138,"titlebarHeight":34,"darkTheme":true,"mounted":true}"#,
            true,
            false,
        ),
        (
            r#"{"schemaVersion":1,"captionWidth":144,"titlebarHeight":34,"darkTheme":true,"mounted":true}"#,
            true,
            false,
        ),
        (
            r#"{"schemaVersion":1,"captionWidth":138,"titlebarHeight":40,"darkTheme":true,"mounted":true}"#,
            true,
            false,
        ),
        (
            r#"{"schemaVersion":1,"captionWidth":138,"titlebarHeight":34,"darkTheme":false,"mounted":true}"#,
            true,
            false,
        ),
        (
            r#"{"schemaVersion":1,"captionWidth":138,"titlebarHeight":34,"darkTheme":true,"mounted":true,"nativePrivilege":true}"#,
            true,
            false,
        ),
        (
            r#"{"schemaVersion":1,"captionWidth":138,"titlebarHeight":34,"darkTheme":true}"#,
            true,
            false,
        ),
    ] {
        let receipt = serde_json::from_str::<Receipt>(json);
        let result = receipt.as_ref().is_ok_and(|value| value.validate(dark).is_ok());
        assert_eq!(result, accepted, "{json}");
    }
}

#[test]
fn configuration_request_is_closed_and_does_not_accept_ready_or_incomplete_receipts() {
    assert!(Needed::decode(r#"{"configure_desktop_frame":1}"#));
    for value in [
        r#"{"configure_desktop_frame":2}"#,
        r#"{"incomplete":1}"#,
        r#"{"configure_desktop_frame":1,"ready":true}"#,
        r#"{"configure_desktop_frame":true}"#,
    ] {
        assert!(!Needed::decode(value));
    }
}

#[test]
fn browser_fills_the_client_from_its_top_row_in_exact_physical_pixels() {
    for (scale, width, height) in [(1.0, 304, 240), (1.25, 1001, 667), (1.75, 2241, 1261)] {
        for top in [0, 34, 60] {
            let bounds = browser_bounds(width, height, top);
            let position = bounds.position.to_physical::<i32>(scale);
            let size = bounds.size.to_physical::<u32>(scale);
            assert_eq!((position.x, position.y), (0, top as i32));
            assert_eq!((size.width, size.height), (width, height - top), "no rounding at {scale}");
        }
    }
    let zero = browser_bounds(0, 0, 0).size.to_physical::<u32>(1.0);
    assert_eq!((zero.width, zero.height), (1, 1));
    // A strip taller than the client still leaves the browser one row.
    let short = browser_bounds(800, 40, 60);
    assert_eq!(short.position.to_physical::<i32>(1.0).y, 39);
    assert_eq!(short.size.to_physical::<u32>(1.0).height, 1);
}

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

/// One 1280 by 800 logical-pixel window's caption peers and page rectangle, laid out by the
/// real caption and browser-bounds owners at device `scale`.
struct Window {
    scale: f64,
    client: [u32; 2],
    controls: [AccessibleControl; PEERS],
    page: RECT,
}

impl Window {
    fn lay_out(caption: &CaptionUpdates, scale: f64) -> TestResult<Self> {
        let client = [(1280.0 * scale).round() as u32, (800.0 * scale).round() as u32];
        let width = f64::from(client[0]) / scale;
        let view = caption.view(messages("en"));
        let retry = Recovery::new(identity()?).presentation(RECOVERY);
        // Seven logical pixels per character stands in for the caption font.
        let measure =
            |text: &str| Ok::<_, std::convert::Infallible>(text.chars().count() as f64 * 7.0);
        let controls = caption_controls(width, false, LABELS, retry, &view, measure)?;
        let bounds = browser_bounds(client[0], client[1], page_top(width, &controls, scale));
        let position = bounds.position.to_physical::<i32>(scale);
        let size = bounds.size.to_physical::<i32>(scale);
        let page = RECT {
            left: position.x,
            top: position.y,
            right: position.x + size.width,
            bottom: position.y + size.height,
        };
        Ok(Self { scale, client, controls, page })
    }

    /// The message line's physical rectangle, where the caption places it.
    fn line(&self) -> TestResult<(bool, RECT)> {
        let line = self.controls.iter().find(|control| control.id == LINE).ok_or("no line")?;
        Ok((line.visible, physical_bounds(line, self.scale)?))
    }

    /// Fail when any shown peer other than the window buttons, which the page's own title bar
    /// leaves free, covers a page pixel.
    fn assert_page_clear(&self) -> TestResult {
        for control in self.controls.iter().filter(|control| control.visible && control.id > 3) {
            let peer = physical_bounds(control, self.scale)?;
            assert!(
                !intersects(peer, self.page),
                "peer {} {peer:?} covers page {:?} at {}",
                control.id,
                self.page,
                self.scale
            );
        }
        Ok(())
    }

    fn full_client(&self) -> RECT {
        RECT { left: 0, top: 0, right: self.client[0] as i32, bottom: self.client[1] as i32 }
    }
}

fn intersects(a: RECT, b: RECT) -> bool {
    a.left < b.right && b.left < a.right && a.top < b.bottom && b.top < a.bottom
}

#[test]
fn the_update_line_never_covers_the_page_and_dismiss_gives_its_strip_back() -> TestResult {
    for scale in SCALES {
        let mut caption = CaptionUpdates::default();
        caption.notify(Notice::Updated {
            version: "0.1.0".to_owned(),
            changelog: Some(url::Url::parse("https://example.invalid/releases/0.1.0")?),
        });
        caption.notify(Notice::NotSaved { workspace: true, inputs: false });

        for _ in 0..2 {
            let shown = Window::lay_out(&caption, scale)?;
            let (visible, line) = shown.line()?;
            assert!(visible, "the line shows at {scale}");
            assert!(
                !intersects(line, shown.page),
                "line {line:?} covers page {:?} at {scale}",
                shown.page
            );
            shown.assert_page_clear()?;
            assert_eq!(shown.page.bottom, shown.client[1] as i32, "page keeps the rest");
            assert!(caption.activate(DISMISS).0, "dismiss shows the next line or none");
        }

        let dismissed = Window::lay_out(&caption, scale)?;
        assert!(!dismissed.line()?.0, "no line after the last dismiss at {scale}");
        dismissed.assert_page_clear()?;
        assert_eq!(dismissed.page, dismissed.full_client(), "the strip returns at {scale}");
    }
    Ok(())
}
