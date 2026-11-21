// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Own source-compatible logical geometry independently of the native window.

use crate::{Bounds, Theme};

/// Preserve the PySide splash's fixed logical width and height.
pub const LOGICAL_SIZE: u32 = 558;

/// Identify a splash-owned control without depending on a native event type.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Control {
    /// Console disclosure immediately beside close.
    ConsoleToggle,
    /// Startup cancellation affordance.
    Close,
}

/// Closed presentation of the actual application recovery phase.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum StartupStatus {
    /// Normal startup is still in progress.
    #[default]
    Loading,
    /// Startup remains active after two minutes; elapsed time grants no failure authority.
    LoadingLong,
    /// Startup remains active after five minutes and may still be preparing dependencies.
    LoadingExtended,
    /// The application retained an original startup failure.
    Failed,
    /// Original attempt cleanup remains unresolved.
    Settling,
    /// Real cleanup settled and the application can accept retry.
    RetryReady,
    /// Cleanup failed and original ownership remains retained.
    CleanupFailed,
    /// A new version is being applied before the application opens.
    Updating(crate::UpdateVersion),
    /// The installed launcher is still handing startup to the application.
    Starting,
    /// The installed launcher is preparing the installation's release again because nothing
    /// installed could start.
    Repairing,
    /// SugarCubes is being updated before the backend loads it.
    UpdatingSugarcubes,
    /// The page failed because SugarCubes could not load, and the application can accept retry.
    SugarcubesRetryReady,
    /// SugarCubes, just updated, could not load; a retry starts the version before the update.
    SugarcubesRetryPrevious,
    /// The start failed and the installed SugarCubes predates the application; the user
    /// updates it, and the application can accept retry.
    SugarcubesOutdated,
    /// The previous SugarCubes version is being put back; retry waits until it is.
    RestoringSugarcubes,
    /// A close waits for a SugarCubes install or restore to end.
    FinishingSugarcubes,
}

impl StartupStatus {
    /// Whether the status line names a settled failed start, which the application retries
    /// by itself.
    pub const fn offers_retry(self) -> bool {
        matches!(
            self,
            Self::RetryReady
                | Self::SugarcubesRetryReady
                | Self::SugarcubesRetryPrevious
                | Self::SugarcubesOutdated
        )
    }
}

/// Supply portable appearance state; process and connection state stay elsewhere.
#[derive(Clone, Copy, Debug)]
pub struct Appearance {
    /// Current presentation theme.
    pub theme: Theme,
    /// Accent roles resolved from the operating-system accent, or the product fallback.
    pub accent: sugarsubstitute_ui::accent::AccentPalette,
    /// Localized application recovery presentation; this grants no cleanup authority.
    pub status: StartupStatus,
    /// Latest observed startup operation, localized independently of elapsed time.
    pub operation: sugarsubstitute_contracts::startup::StartupOperation,
    /// Requested disclosure state used by the caption affordance.
    pub expanded: bool,
    /// Sampled console expansion in the inclusive range zero to one.
    pub reveal: f64,
    /// Control under the pointer.
    pub hovered: Option<Control>,
    /// Currently pressed control.
    pub pressed: Option<Control>,
    /// Whether the console and close controls are drawn. The launcher's notice has neither:
    /// it has no output to show and ends by itself within seconds.
    pub controls: bool,
    /// Whether the output disclosure is offered. An attached ComfyUI writes no output to this
    /// application, so its splash has nothing to disclose.
    pub output: bool,
    /// The reason the last start failed, named in the status line while the start is retried.
    pub failure: Option<sugarsubstitute_contracts::startup::StartupFailureCode>,
    /// Whether the status line says the failed start is being tried again by itself.
    pub retrying: bool,
}

impl Appearance {
    /// Open the requested blank-console proof with the source dark theme and product accent.
    pub const fn demo() -> Self {
        Self {
            theme: Theme::Dark,
            accent: sugarsubstitute_ui::accent::AccentPalette::PRODUCT,
            status: StartupStatus::Loading,
            operation: sugarsubstitute_contracts::startup::StartupOperation::Preparing,
            expanded: true,
            reveal: 1.0,
            hovered: None,
            pressed: None,
            controls: true,
            output: true,
            failure: None,
            retrying: false,
        }
    }

    /// Return the source widget's whole mascot slot before aspect fitting.
    pub const fn mascot(self) -> Bounds {
        Bounds {
            x: 85.0 - 2.0 * self.reveal,
            y: 86.0 - 79.0 * self.reveal,
            width: 387.0,
            height: 386.0,
        }
    }

    /// Return the console panel growing upward from the fixed lower edge.
    pub const fn feedback(self) -> Bounds {
        Bounds { x: 24.0, y: self.bar_y(), width: 510.0, height: 534.0 - self.bar_y() }
    }

    /// Keep the single progress bar attached to the moving console edge.
    pub const fn bar_y(self) -> f64 {
        530.0 - 174.0 * self.reveal
    }

    /// Simplified feedback appears only after the full console finishes closing.
    pub fn status_visible(self) -> bool {
        !self.expanded && self.reveal == 0.0
    }

    /// Whether the failed start named in the status line is retried by itself. A ComfyUI that
    /// serves another frontend version or no SugarCubes, or a SugarCubes too old for this
    /// application, stays that way until the person fixes it, so those ask instead.
    pub fn retries_by_itself(self) -> bool {
        use sugarsubstitute_contracts::startup::StartupFailureCode;
        match self.status {
            StartupStatus::RetryReady => !matches!(
                self.failure,
                Some(StartupFailureCode::FrontendMismatch | StartupFailureCode::SugarCubesMissing)
            ),
            StartupStatus::SugarcubesRetryReady | StartupStatus::SugarcubesRetryPrevious => true,
            _ => false,
        }
    }

    /// Whether the failed start named in the status line needs the person: a dialog explains
    /// it and offers Retry and Close.
    pub fn asks_about_failure(self) -> bool {
        self.status.offers_retry() && !self.retries_by_itself()
    }
}

#[cfg(test)]
mod tests {
    use super::Appearance;

    /// Keep disclosure's two reference positions independent of presentation timers.
    #[test]
    fn disclosure_reflows_only_the_reference_geometry() {
        let mut appearance = Appearance::demo();
        assert_eq!(appearance.mascot().y, 7.0);
        assert_eq!(appearance.feedback().height, 178.0);
        appearance.expanded = false;
        appearance.reveal = 0.0;
        assert_eq!(appearance.mascot().y, 86.0);
        assert_eq!(appearance.feedback().y, 530.0);
    }
}
