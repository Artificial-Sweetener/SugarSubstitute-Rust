// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Compose the report body (glyph, heading, message, summary card, details toggle and panel,
//! dump consent) in window coordinates, and the shared builder used for every part.
use super::layout::*;
use super::{CommandState, DetailsText, ReportAction, ReportContent, ReportLabels, ReportState};
use crate::{
    form::{Control, Role},
    icons::FluentIcon,
    scene::Element,
    text::{LabelLayout, TextAlignment},
    widgets::{Bounds, ControlState, Decoration, TextTone},
};
use sugarsubstitute_contracts::configuration::Theme;

/// Composition scratch for one part of a frame.
pub(super) struct Builder<'a> {
    pub(super) theme: Theme,
    pub(super) labels: &'a ReportLabels,
    pub(super) state: &'a ReportState,
    pub(super) elements: Vec<Element>,
    pub(super) controls: Vec<Control>,
}

/// Where the details panel's text lines go.
pub(super) enum Lines {
    /// Into a separate layer clipped to the panel and scrolled by `scroll` (the window fits).
    Panel {
        /// Requested text scroll offset.
        scroll: f32,
    },
    /// Into the body itself, limited to the window-coordinate band `[top, bottom)`; the whole
    /// body scrolls (the window is shorter than the report).
    Body {
        /// Top of the visible band.
        top: f32,
        /// Bottom of the visible band.
        bottom: f32,
    },
}

/// The details panel's own clipped layer, when its text scrolls inside it.
pub(super) struct Panel {
    pub(super) viewport: Bounds,
    pub(super) elements: Vec<Element>,
    pub(super) content_height: f32,
    pub(super) scroll: f32,
}

impl<'a> Builder<'a> {
    pub(super) fn new(theme: Theme, labels: &'a ReportLabels, state: &'a ReportState) -> Self {
        Self { theme, labels, state, elements: Vec::new(), controls: Vec::new() }
    }
    pub(super) fn text(&mut self, text: &str, layout: LabelLayout) {
        if !text.is_empty() {
            self.elements.push(Element::Label { text: text.to_owned(), layout });
        }
    }
    pub(super) fn peer(&mut self, action: ReportAction, bounds: Bounds) -> ControlState {
        self.peer_role(action, bounds, Role::Button)
    }
    fn peer_role(&mut self, action: ReportAction, bounds: Bounds, role: Role) -> ControlState {
        let command = self.state.command(action);
        let mut control = peer(action, self.labels, self.state, bounds, command);
        control.role = role;
        self.controls.push(control);
        visual_state(self.state, action, command)
    }
    pub(super) fn button(
        &mut self,
        action: ReportAction,
        bounds: Bounds,
        primary: bool,
        text: &str,
    ) {
        let state = self.peer(action, bounds);
        self.elements.push(Element::Decoration {
            bounds,
            appearance: Decoration::Button { primary, state },
        });
        let mut layout = label(
            bounds.x + 12.0,
            bounds.y + 6.0,
            bounds.width - 24.0,
            19.0,
            14.0,
            19.0,
            tone(TextTone::Action { primary, state }, self.theme),
        );
        layout.alignment = TextAlignment::Center;
        self.text(text, layout);
    }
    fn tone(&self, tone: TextTone) -> [u8; 4] {
        tone.color(self.theme)
    }
}

impl ReportLayout {
    /// Compose the body below the title bar. `opening` is the space the details panel and its
    /// spacing take at the current expansion. Returns the panel's own layer in
    /// [`Lines::Panel`] mode.
    pub(super) fn body(
        &self,
        b: &mut Builder<'_>,
        content: &ReportContent,
        details: Option<&DetailsText>,
        opening: f32,
        lines: Lines,
    ) -> Option<Panel> {
        let top = TITLE_BAR + TOP;
        let rows = self.header().min(52.0);
        b.elements.push(Element::Icon {
            bounds: rect(MARGIN, top + ((rows - GLYPH) * 0.5).max(0.0), GLYPH, GLYPH),
            icon: content.severity.icon(),
            on_accent: false,
        });
        let text_x = MARGIN + GLYPH + GLYPH_GAP;
        let text_width = WIDTH - MARGIN - text_x;
        let mut heading =
            label(text_x, top, text_width, self.title, 20.0, 28.0, b.tone(TextTone::Primary));
        heading.weight = 600;
        b.text(&content.title, heading);
        if self.message > 0.0 {
            let tone = b.tone(TextTone::Secondary);
            let y = top + self.title + 4.0;
            b.text(&content.message, label(text_x, y, text_width, self.message, 14.0, 20.0, tone));
        }
        let mut y = top + self.header();
        if !self.summary.is_empty() {
            y += SPACING;
            self.summary_card(b, content, y);
            y += self.summary_height();
        }
        let mut panel_layer = None;
        if self.details > 0.0 {
            y += SPACING;
            let toggle = rect(MARGIN, y, WIDTH - 2.0 * MARGIN, BUTTON);
            let text = b.labels.action(ReportAction::ToggleDetails, b.state.expanded);
            b.button(ReportAction::ToggleDetails, toggle, false, text);
            y += BUTTON;
            let reveal = opening - SPACING;
            if reveal >= 2.0 {
                let panel = rect(MARGIN, y + SPACING, WIDTH - 2.0 * MARGIN, reveal);
                panel_layer = Self::details_panel_content(b, panel, details, lines);
            }
            y += opening;
        }
        if let Some((label_height, warning)) = self.consent {
            y += SPACING;
            self.consent_row(b, y, label_height, warning);
        }
        panel_layer
    }

    fn summary_card(&self, b: &mut Builder<'_>, content: &ReportContent, y: f32) {
        let card = rect(MARGIN, y, WIDTH - 2.0 * MARGIN, self.summary_height());
        b.elements.push(Element::Decoration { bounds: card, appearance: Decoration::Card });
        let mut row_y = y + 10.0;
        let value_x = MARGIN + 12.0 + self.summary_label + 18.0;
        let (secondary, primary) = (b.tone(TextTone::Secondary), b.tone(TextTone::Primary));
        for (row, (label_height, value_height)) in content.summary.iter().zip(&self.summary) {
            let name_width = self.summary_label.max(1.0);
            b.text(
                &row.label,
                label(MARGIN + 12.0, row_y + 2.0, name_width, *label_height, 12.0, 16.0, secondary),
            );
            let value_width = WIDTH - MARGIN - 12.0 - value_x;
            let mut value = label(value_x, row_y, value_width, *value_height, 14.0, 20.0, primary);
            value.weight = 600;
            b.text(&row.value, value);
            row_y += label_height.max(*value_height) + 6.0;
        }
    }

    fn details_panel_content(
        b: &mut Builder<'_>,
        panel: Bounds,
        details: Option<&DetailsText>,
        lines: Lines,
    ) -> Option<Panel> {
        let field =
            Decoration::Field { selection: false, focused: false, state: ControlState::Rest };
        let color = b.tone(TextTone::Primary);
        let text_width = Self::details_text_width();
        match lines {
            Lines::Panel { scroll } => {
                b.elements.push(Element::Decoration { bounds: panel, appearance: field });
                let viewport =
                    rect(panel.x + 1.0, panel.y + 1.0, panel.width - 2.0, panel.height - 2.0);
                let measured = details.map_or(0.0, DetailsText::height);
                let content_height = (measured + 2.0 * DETAILS_PADDING).max(viewport.height);
                let scroll = scroll.clamp(0.0, content_height - viewport.height);
                let mut elements = Vec::new();
                if let Some(details) = details {
                    let top = scroll - DETAILS_PADDING;
                    for (text, line_y, height) in details.visible(top, top + viewport.height) {
                        let mut layout =
                            DetailsText::layout(line_y - top - 1.0, text_width, height, color);
                        layout.origin[0] = DETAILS_PADDING - 1.0;
                        if !text.is_empty() {
                            elements.push(Element::Label { text: text.to_owned(), layout });
                        }
                    }
                }
                Some(Panel { viewport, elements, content_height, scroll })
            }
            Lines::Body { top, bottom } => {
                // Only the visible part of a tall panel is drawn; its edges stay out of view.
                let visible_top = panel.y.max(top - 8.0);
                let visible_bottom = (panel.y + panel.height).min(bottom + 8.0);
                if visible_bottom > visible_top {
                    let bounds =
                        rect(panel.x, visible_top, panel.width, visible_bottom - visible_top);
                    b.elements.push(Element::Decoration { bounds, appearance: field });
                }
                if let Some(details) = details {
                    let origin = panel.y + DETAILS_PADDING;
                    for (text, line_y, height) in details.visible(top - origin, bottom - origin) {
                        if line_y + DETAILS_PADDING + height > panel.height {
                            break;
                        }
                        let mut layout =
                            DetailsText::layout(origin + line_y, text_width, height, color);
                        layout.origin[0] = panel.x + DETAILS_PADDING;
                        if !text.is_empty() {
                            b.elements.push(Element::Label { text: text.to_owned(), layout });
                        }
                    }
                }
                None
            }
        }
    }

    fn consent_row(&self, b: &mut Builder<'_>, y: f32, label_height: f32, warning: f32) {
        let check = rect(MARGIN, y + (label_height.min(20.0) - CHECK) * 0.5, CHECK, CHECK);
        let row = rect(MARGIN, y, WIDTH - 2.0 * MARGIN, label_height);
        let checked = b.state.include_dump;
        let state = b.peer_role(ReportAction::IncludeDump, row, Role::Check(checked));
        b.elements.push(Element::Decoration {
            bounds: check,
            appearance: Decoration::Check { checked, state },
        });
        if checked {
            b.elements.push(Element::Icon {
                bounds: rect(check.x + 4.0, check.y + 4.0, 10.0, 10.0),
                icon: FluentIcon::Accept,
                on_accent: true,
            });
        }
        let x = MARGIN + CHECK + 9.0;
        let w = WIDTH - MARGIN - x;
        let (primary, error) = (b.tone(TextTone::Primary), b.tone(TextTone::Error));
        b.text(b.labels.include_dump, label(x, y, w, label_height, 14.0, 20.0, primary));
        b.text(
            b.labels.dump_warning,
            label(x, y + label_height + 4.0, w, warning, 12.0, 16.0, error),
        );
    }
}

/// Application availability takes precedence over observed pointer state.
fn visual_state(state: &ReportState, action: ReportAction, command: CommandState) -> ControlState {
    let id = Some(action.id());
    if !command.enabled {
        ControlState::Disabled
    } else if state.interaction.pressed == id {
        ControlState::Pressed
    } else if state.interaction.hovered == id {
        ControlState::Hover
    } else {
        ControlState::Rest
    }
}
