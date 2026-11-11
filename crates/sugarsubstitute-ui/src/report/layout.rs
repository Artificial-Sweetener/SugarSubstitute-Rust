// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Measure and compose the report: caption, severity glyph, heading, message, summary card,
//! details toggle and text, dump consent, and the footer command band.
use super::{CommandState, ReportAction, ReportContent, ReportKind, ReportLabels, ReportState};
use crate::{
    availability::Availability,
    form::{Control, Role, SCROLL_GUTTER},
    scene::SceneResources,
    text::{LabelLayout, LabelPaintSpec, PaintSpec, TextAlignment, TextMetrics, VerticalAnchor},
    widgets::{Bounds, TextTone},
};
use std::io;
use sugarsubstitute_contracts::{configuration::Theme, execution::MemoryLease};

/// Fixed logical report width.
pub const WIDTH: f32 = 720.0;
pub(super) const TITLE_BAR: f32 = 32.0;
pub(super) const CLOSE_WIDTH: f32 = 46.0;
pub(super) const MARGIN: f32 = 24.0;
pub(super) const TOP: f32 = 24.0;
pub(super) const BOTTOM: f32 = 16.0;
pub(super) const SPACING: f32 = 12.0;
pub(super) const FOOTER: f32 = 68.0;
pub(super) const BUTTON: f32 = 32.0;
pub(super) const GLYPH: f32 = 32.0;
pub(super) const GLYPH_GAP: f32 = 12.0;
pub(super) const DETAILS: f32 = 260.0;
pub(super) const DETAILS_COMPACT: f32 = 160.0;
pub(super) const DETAILS_PADDING: f32 = 8.0;
pub(super) const CHECK: f32 = 18.0;

/// Measured text geometry for one content revision at one display scale.
pub struct ReportLayout {
    pub(super) title: f32,
    pub(super) message: f32,
    pub(super) summary: Vec<(f32, f32)>,
    pub(super) summary_label: f32,
    pub(super) consent: Option<(f32, f32)>,
    pub(super) buttons: Vec<(ReportAction, f32)>,
    pub(super) details: f32,
}

impl ReportLayout {
    /// The stable peer inventory of a report kind. Commands a kind never offers are absent;
    /// commands offered later (Restart, Force) start hidden.
    pub fn inventory(kind: ReportKind, labels: &ReportLabels) -> Vec<Control> {
        let state = ReportState::new(kind);
        let parked = Bounds { x: 0.0, y: 0.0, width: 1.0, height: 1.0 };
        let mut actions = vec![ReportAction::Dismiss, ReportAction::ToggleDetails];
        if kind.offers_dump() {
            actions.push(ReportAction::IncludeDump);
        }
        actions.extend_from_slice(kind.commands());
        actions
            .into_iter()
            .map(|action| peer(action, labels, &state, parked, CommandState::default()))
            .collect()
    }

    /// Measure every label of `content` with the font owner that will paint it. The details
    /// height shrinks when the fully expanded report would exceed `maximum_height`.
    /// # Errors
    /// Preserves font and memory-admission failures.
    pub fn measure(
        content: &ReportContent,
        labels: &ReportLabels,
        state: &ReportState,
        surface: PaintSpec,
        resources: &mut SceneResources,
        lease: Option<&MemoryLease>,
        maximum_height: f32,
    ) -> io::Result<Self> {
        let mut measure = |text: &str, width: f32, size: f32, line: f32, weight: u16| {
            let mut layout = label(0.0, 0.0, width, 8192.0, size, line, [0; 4]);
            layout.weight = weight;
            resources.measure(text, LabelPaintSpec { surface, layout }, lease)
        };
        let text_width = WIDTH - 2.0 * MARGIN - GLYPH - GLYPH_GAP;
        let title = measure(&content.title, text_width, 20.0, 28.0, 600)?.height.max(28.0);
        let message = if content.message.is_empty() {
            0.0
        } else {
            measure(&content.message, text_width, 14.0, 20.0, 400)?.height
        };
        let mut summary_label: f32 = 0.0;
        for row in &content.summary {
            summary_label = summary_label.max(measure(&row.label, 200.0, 12.0, 16.0, 400)?.width);
        }
        summary_label = summary_label.ceil().min(200.0);
        let value_width = WIDTH - 2.0 * MARGIN - 24.0 - summary_label - 18.0;
        let mut summary = Vec::with_capacity(content.summary.len());
        for row in &content.summary {
            let label = measure(&row.label, summary_label.max(1.0), 12.0, 16.0, 400)?.height;
            let value = measure(&row.value, value_width, 14.0, 20.0, 600)?.height.max(20.0);
            summary.push((label, value));
        }
        let consent = if content.kind.offers_dump() {
            let width = WIDTH - 2.0 * MARGIN - CHECK - 9.0;
            Some((
                measure(labels.include_dump, width, 14.0, 20.0, 400)?.height.max(20.0),
                measure(labels.dump_warning, width, 12.0, 16.0, 400)?.height,
            ))
        } else {
            None
        };
        let mut buttons = Vec::new();
        for action in content.kind.commands() {
            if state.command(*action).visible {
                let minimum = if *action == ReportAction::Close { 88.0 } else { 108.0 };
                let text = labels.action(*action, false);
                let width = (measure(text, 4096.0, 14.0, 19.0, 400)?.width + 26.0).ceil();
                buttons.push((*action, width.max(minimum)));
            }
        }
        let mut layout = Self {
            title,
            message,
            summary,
            summary_label,
            consent,
            buttons,
            details: if content.has_details { DETAILS } else { 0.0 },
        };
        if content.has_details && layout.height(1.0) > maximum_height {
            layout.details = DETAILS_COMPACT;
        }
        Ok(layout)
    }

    /// Natural window height at a details expansion between 0 and 1.
    pub fn height(&self, expansion: f32) -> f32 {
        self.body_end(expansion) + BOTTOM + FOOTER
    }

    /// Logical details panel inside the window at an expansion.
    pub fn details_panel(&self, expansion: f32) -> Bounds {
        let top = self.toggle_top() + BUTTON + SPACING;
        Bounds { x: MARGIN, y: top, width: WIDTH - 2.0 * MARGIN, height: self.reveal(expansion) }
    }

    /// Width available to details text lines.
    pub fn details_text_width() -> f32 {
        WIDTH - 2.0 * MARGIN - 2.0 * DETAILS_PADDING - SCROLL_GUTTER
    }

    pub(super) fn header(&self) -> f32 {
        self.title + if self.message > 0.0 { 4.0 + self.message } else { 0.0 }
    }
    pub(super) fn summary_height(&self) -> f32 {
        if self.summary.is_empty() {
            return 0.0;
        }
        let rows: f32 = self.summary.iter().map(|(l, v)| l.max(*v)).sum();
        20.0 + rows + 6.0 * (self.summary.len() - 1) as f32
    }
    fn toggle_top(&self) -> f32 {
        let mut y = TITLE_BAR + TOP + self.header();
        if !self.summary.is_empty() {
            y += SPACING + self.summary_height();
        }
        y + SPACING
    }
    fn reveal(&self, expansion: f32) -> f32 {
        ((SPACING + self.details) * expansion.clamp(0.0, 1.0) - SPACING).max(0.0)
    }
    pub(super) fn body_end(&self, expansion: f32) -> f32 {
        let mut y = TITLE_BAR + TOP + self.header();
        if !self.summary.is_empty() {
            y += SPACING + self.summary_height();
        }
        if self.details > 0.0 {
            y += SPACING + BUTTON + (SPACING + self.details) * expansion.clamp(0.0, 1.0);
        }
        if let Some((label, warning)) = self.consent {
            y += SPACING + label + 4.0 + warning;
        }
        y
    }
}

/// One accessible peer; its availability follows the command state.
pub(super) fn peer(
    action: ReportAction,
    labels: &ReportLabels,
    state: &ReportState,
    bounds: Bounds,
    command: CommandState,
) -> Control {
    Control {
        id: action.id(),
        label: labels.action(action, state.expanded),
        bounds,
        availability: Availability { visible: command.visible, enabled: command.enabled },
        role: if action == ReportAction::IncludeDump {
            Role::Check(state.include_dump)
        } else {
            Role::Button
        },
        text_revision: 0,
    }
}

pub(super) fn rect(x: f32, y: f32, width: f32, height: f32) -> Bounds {
    Bounds { x, y, width, height }
}

pub(super) fn tone(tone: TextTone, theme: Theme) -> [u8; 4] {
    tone.color(theme)
}

pub(super) fn label(
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    size: f32,
    line: f32,
    color: [u8; 4],
) -> LabelLayout {
    LabelLayout {
        origin: [x, y],
        extent: [width.max(1.0), height.max(line)],
        clip: None,
        anchor: VerticalAnchor::Top,
        alignment: TextAlignment::Leading,
        metrics: TextMetrics::Natural,
        font_size: size,
        line_height: line,
        weight: 400,
        color,
    }
}
