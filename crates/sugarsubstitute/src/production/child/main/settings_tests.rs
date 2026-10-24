// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Behavioral fixtures for the owning presentation contract.
use super::*;
use sugarsubstitute_contracts::configuration::{Presentation, Theme};
#[test]
fn authenticated_presentation_controls_caption_and_theme_without_changing_geometry() -> Result<()> {
    for locale in sugarsubstitute_contracts::localization::LOCALES {
        for theme in [Theme::Light, Theme::Dark] {
            let settings = requested_settings(
                MainWindowSettings {
                    recovery: sugarsubstitute_desktop::MainRecoveryLabels {
                        retry: sugarsubstitute_contracts::localization::messages("en").retry,
                        retry_ready: sugarsubstitute_contracts::localization::messages("en")
                            .retry_ready,
                    },
                    logical_width: 1280,
                    logical_height: 720,
                    dark: true,
                    labels: sugarsubstitute_desktop::MainCaptionLabels {
                        minimize: "initial",
                        maximize: "initial",
                        restore: "initial",
                        close: "initial",
                    },
                },
                Presentation::new(locale.id, theme, false)?,
            );
            let copy = sugarsubstitute_contracts::localization::messages(locale.id);
            let expected = copy.window;
            assert_eq!(
                (settings.recovery.retry, settings.recovery.retry_ready),
                (copy.retry, copy.retry_ready),
            );
            assert_eq!((settings.logical_width, settings.logical_height), (1280, 720));
            assert_eq!(settings.dark, theme == Theme::Dark);
            assert_eq!(
                [
                    settings.labels.minimize,
                    settings.labels.maximize,
                    settings.labels.restore,
                    settings.labels.close
                ],
                [expected.minimize, expected.maximize, expected.restore, expected.close]
            );
        }
    }
    Ok(())
}
