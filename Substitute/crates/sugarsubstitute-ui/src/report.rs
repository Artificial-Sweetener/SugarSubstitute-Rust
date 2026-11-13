// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! One report surface for every native failure presentation: crash, startup failure, an
//! unresponsive previous window, and update failure. The model is plain data; composition produces shared scene
//! commands and an accessible peer inventory. Native windows, clipboard and files stay with the
//! desktop adapter, which hosts this view without interpreting its layout.

use crate::form::ControlId;
use std::num::NonZeroU16;

mod body;
mod compose;
mod details;
mod layout;
mod motion;
#[cfg(test)]
mod tests;

pub use compose::ReportFrame;
pub use details::{DetailsLine, DetailsText};
pub use layout::{ReportLayout, WIDTH};
pub use motion::{DetailsMotion, EXPAND_DURATION};

/// Visual tone of the report's severity glyph.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReportSeverity {
    /// Something failed and the user needs to know.
    Error,
    /// Something needs attention but nothing failed yet.
    Warning,
    /// Neutral information.
    Info,
}

impl ReportSeverity {
    /// The bundled glyph for this severity.
    pub const fn icon(self) -> crate::icons::FluentIcon {
        match self {
            Self::Error => crate::icons::FluentIcon::SeverityError,
            Self::Warning => crate::icons::FluentIcon::SeverityWarning,
            Self::Info => crate::icons::FluentIcon::SeverityInfo,
        }
    }
}

/// The failure a report presents; each kind fixes its command set.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReportKind {
    /// A previous run ended abnormally. Offers dump consent, Export and an optional Restart.
    Crash,
    /// This launch could not start.
    StartupFailure,
    /// The previous window stopped responding; the user may close it and start, or cancel.
    UnresponsiveCopy,
    /// An update attempt failed. Copy and Export only; nothing restarts or uploads.
    UpdateFailure,
}

impl ReportKind {
    /// Footer commands from left to right. Unavailable commands stay in the inventory hidden.
    pub const fn commands(self) -> &'static [ReportAction] {
        match self {
            Self::Crash => &[
                ReportAction::Copy,
                ReportAction::Export,
                ReportAction::Close,
                ReportAction::Restart,
            ],
            Self::StartupFailure => &[ReportAction::Copy, ReportAction::Close],
            Self::UnresponsiveCopy => &[ReportAction::Cancel, ReportAction::Close],
            Self::UpdateFailure => &[ReportAction::Copy, ReportAction::Export, ReportAction::Close],
        }
    }

    /// The command the window's close button and Escape stand for.
    pub const fn dismissal(self) -> ReportAction {
        match self {
            Self::UnresponsiveCopy => ReportAction::Cancel,
            _ => ReportAction::Close,
        }
    }

    /// Whether the report offers explicit consent to include a native memory dump in Export.
    pub const fn offers_dump(self) -> bool {
        matches!(self, Self::Crash)
    }
}

/// One user command. Identities are stable for the surface's lifetime.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReportAction {
    /// The caption close button; resolved through [`ReportKind::dismissal`].
    Dismiss,
    /// Show or hide the complete report text.
    ToggleDetails,
    /// Explicit consent to include a native memory dump in Export.
    IncludeDump,
    /// Copy the complete report text.
    Copy,
    /// Save the report to a file the user selects.
    Export,
    /// Close the report (for an unresponsive previous window: close it and start).
    Close,
    /// Close and request a fresh start.
    Restart,
    /// Leave the previous window alone and cancel this start.
    Cancel,
}

impl ReportAction {
    /// Every action in inventory order.
    pub const ALL: [Self; 8] = [
        Self::Dismiss,
        Self::ToggleDetails,
        Self::IncludeDump,
        Self::Copy,
        Self::Export,
        Self::Close,
        Self::Restart,
        Self::Cancel,
    ];

    /// Native peer identity. Four is reserved for the shared console peer and never used.
    pub const fn id(self) -> ControlId {
        let value = match self {
            Self::Dismiss => 1,
            Self::ToggleDetails => 2,
            Self::IncludeDump => 3,
            Self::Copy => 5,
            Self::Export => 6,
            Self::Close => 7,
            Self::Restart => 8,
            Self::Cancel => 10,
        };
        // Every literal above is nonzero, so the fallback arm is never taken.
        ControlId::from_nonzero(match NonZeroU16::new(value) {
            Some(value) => value,
            None => NonZeroU16::MAX,
        })
    }

    /// Resolve a native peer identity back to its action.
    pub fn from_id(id: ControlId) -> Option<Self> {
        Self::ALL.into_iter().find(|action| action.id() == id)
    }
}

/// Localized command and accessibility text, borrowed from the localization owner.
#[derive(Clone, Copy, Debug)]
pub struct ReportLabels {
    /// Accessible name of the caption close button.
    pub close_window: &'static str,
    /// Reveal the complete report.
    pub show_report: &'static str,
    /// Collapse the complete report.
    pub hide_report: &'static str,
    /// Copy command.
    pub copy: &'static str,
    /// Export command.
    pub export: &'static str,
    /// Close command; for an unresponsive previous window, "close it and start".
    pub close: &'static str,
    /// Restart command.
    pub restart: &'static str,
    /// Cancel command.
    pub cancel: &'static str,
    /// Dump consent label.
    pub include_dump: &'static str,
    /// Dump disclosure shown beside the consent.
    pub dump_warning: &'static str,
}

impl ReportLabels {
    /// The command text for an action.
    pub const fn action(&self, action: ReportAction, expanded: bool) -> &'static str {
        match action {
            ReportAction::Dismiss => self.close_window,
            ReportAction::ToggleDetails if expanded => self.hide_report,
            ReportAction::ToggleDetails => self.show_report,
            ReportAction::IncludeDump => self.include_dump,
            ReportAction::Copy => self.copy,
            ReportAction::Export => self.export,
            ReportAction::Close => self.close,
            ReportAction::Restart => self.restart,
            ReportAction::Cancel => self.cancel,
        }
    }
}

/// One fact shown in the summary card; values are shown exactly as supplied.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SummaryRow {
    /// Localized label.
    pub label: String,
    /// Fact text.
    pub value: String,
}

/// Everything the report says. Text is already redacted by its producer.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReportContent {
    /// Which failure this is.
    pub kind: ReportKind,
    /// Glyph tone.
    pub severity: ReportSeverity,
    /// Window title, shown in the caption.
    pub window_title: String,
    /// Heading beside the glyph.
    pub title: String,
    /// Explanation below the heading.
    pub message: String,
    /// Summary card rows; an empty list omits the card.
    pub summary: Vec<SummaryRow>,
    /// Whether the full report text exists; when false the details toggle is omitted.
    pub has_details: bool,
}

/// Visibility and availability of one command.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct CommandState {
    /// Shown in the footer.
    pub visible: bool,
    /// Accepts input.
    pub enabled: bool,
}

/// A short result line in the footer, such as "Report copied.".
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReportStatus {
    /// Localized status text.
    pub text: String,
    /// Render with the error tone.
    pub error: bool,
}

/// Pointer and keyboard facts observed by the native host.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Interaction {
    /// Peer under the pointer.
    pub hovered: Option<ControlId>,
    /// Peer holding the press gesture.
    pub pressed: Option<ControlId>,
    /// Peer holding keyboard focus.
    pub focused: Option<ControlId>,
}

/// Mutable presentation state owned by the hosting surface.
#[derive(Clone, Debug, PartialEq)]
pub struct ReportState {
    commands: [CommandState; 8],
    /// Dump consent; reset whenever the report text is replaced.
    pub include_dump: bool,
    /// Current details expansion, 0 collapsed through 1 expanded.
    pub expansion: f32,
    /// Whether the details are expanded or expanding.
    pub expanded: bool,
    /// Observed native interaction.
    pub interaction: Interaction,
    /// Details scroll offset in logical pixels.
    pub scroll: f32,
    /// Footer status line.
    pub status: Option<ReportStatus>,
}

impl ReportState {
    /// Every footer command of `kind` is visible and enabled, except Restart, which appears
    /// only when its owner offers it.
    pub fn new(kind: ReportKind) -> Self {
        let mut state = Self {
            commands: [CommandState::default(); 8],
            include_dump: false,
            expansion: 0.0,
            expanded: false,
            interaction: Interaction::default(),
            scroll: 0.0,
            status: None,
        };
        let on = CommandState { visible: true, enabled: true };
        state.set(ReportAction::Dismiss, on);
        state.set(ReportAction::ToggleDetails, on);
        state.set(
            ReportAction::IncludeDump,
            CommandState { visible: kind.offers_dump(), enabled: true },
        );
        for action in kind.commands() {
            let offered = *action != ReportAction::Restart;
            state.set(*action, CommandState { visible: offered, enabled: offered });
        }
        state
    }

    /// Current state of one command.
    pub fn command(&self, action: ReportAction) -> CommandState {
        self.commands[Self::slot(action)]
    }

    /// Replace one command's state.
    pub fn set(&mut self, action: ReportAction, state: CommandState) {
        self.commands[Self::slot(action)] = state;
    }

    /// Enable or disable every visible command at once, for example while a request is pending.
    pub fn set_all_enabled(&mut self, enabled: bool) {
        for command in &mut self.commands {
            command.enabled = enabled && command.visible;
        }
    }

    /// The emphasized footer command: Restart when it is offered, otherwise Close.
    pub fn primary(&self) -> ReportAction {
        if self.command(ReportAction::Restart).visible {
            ReportAction::Restart
        } else {
            ReportAction::Close
        }
    }

    fn slot(action: ReportAction) -> usize {
        ReportAction::ALL.iter().position(|item| *item == action).unwrap_or(0)
    }
}
