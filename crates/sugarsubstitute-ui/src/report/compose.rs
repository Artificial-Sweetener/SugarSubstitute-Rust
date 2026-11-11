// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Compose one measured report into scene commands and its accessible peers.
use super::layout::*;
use super::{CommandState, DetailsText, ReportAction, ReportContent, ReportLabels, ReportState};
use crate::{
    accent::AccentPalette,
    form::{Control, Role},
    icons::FluentIcon,
    scene::{Element, FramePaint, Layer},
    text::{LabelLayout, PaintSpec, TextAlignment},
    widgets::{Bounds, ControlState, Decoration, TextTone},
};
use std::io;
use sugarsubstitute_contracts::{
    configuration::Theme, execution::MemoryLease, presentation::FrameIdentity,
};

/// Scene commands and native peers for exactly one prepared frame.
pub struct ReportFrame {
    /// Whole-window layer.
    pub shell: Vec<Element>,
    /// Details layer, relative to `viewport` and already displaced by `scroll`.
    pub page: Vec<Element>,
    /// Window-relative peers for this frame.
    pub controls: Vec<Control>,
    /// Logical details viewport inside the window.
    pub viewport: Bounds,
    /// Logical caption drag region.
    pub drag: Bounds,
    /// Full details content height, for the host scrollbar.
    pub content_height: f32,
    /// Clamped details scroll offset.
    pub scroll: f32,
}

impl ReportFrame {
    /// Hand both layers to the shared painter.
    /// # Errors
    /// Preserves geometry and memory-admission failures.
    pub fn into_paint(
        self,
        surface: PaintSpec,
        accent: AccentPalette,
        identity: FrameIdentity,
        lease: MemoryLease,
    ) -> io::Result<FramePaint> {
        let whole = Bounds {
            x: 0.0,
            y: 0.0,
            width: surface.width as f32 / surface.scale,
            height: surface.height as f32 / surface.scale,
        };
        FramePaint::new(
            surface,
            accent,
            identity,
            vec![
                Layer { viewport: whole, elements: self.shell },
                Layer { viewport: self.viewport, elements: self.page },
            ],
            lease,
        )
    }
}

impl ReportLayout {
    /// Compose one frame at `surface`, whose logical height must fit the natural height.
    /// `details` supplies the measured lines; unmeasured lines are not painted yet.
    /// # Errors
    /// Rejects a surface narrower than the report or shorter than its natural height.
    pub fn compose(
        &self,
        content: &ReportContent,
        labels: &ReportLabels,
        state: &ReportState,
        surface: PaintSpec,
        details: Option<&DetailsText>,
    ) -> io::Result<ReportFrame> {
        let width = surface.width as f32 / surface.scale;
        let height = surface.height as f32 / surface.scale;
        if width + 0.5 < WIDTH || height + 0.5 < self.height(state.expansion) {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, "Report surface too small"));
        }
        let theme = surface.theme;
        let mut b = Builder { theme, labels, state, shell: Vec::new(), controls: Vec::new() };
        // Caption: title and close; the rest of the strip moves the window.
        let caption = label(
            16.0,
            9.0,
            WIDTH - CLOSE_WIDTH - 32.0,
            16.0,
            12.0,
            16.0,
            tone(TextTone::Primary, theme),
        );
        b.text(&content.window_title, caption);
        let close = rect(WIDTH - CLOSE_WIDTH, 0.0, CLOSE_WIDTH, TITLE_BAR);
        let dismiss = b.peer(ReportAction::Dismiss, close);
        b.shell.push(Element::Decoration {
            bounds: close,
            appearance: Decoration::Close { state: dismiss },
        });
        let glyph =
            if dismiss == ControlState::Rest { tone(TextTone::Primary, theme) } else { [255; 4] };
        b.shell.push(Element::Glyph {
            bounds: rect(WIDTH - 28.0, 11.0, 10.0, 10.0),
            icon: FluentIcon::Close,
            color: glyph,
        });
        // Header: glyph centered on the first two text rows.
        let top = TITLE_BAR + TOP;
        let rows = self.header().min(52.0);
        b.shell.push(Element::Icon {
            bounds: rect(MARGIN, top + ((rows - GLYPH) * 0.5).max(0.0), GLYPH, GLYPH),
            icon: content.severity.icon(),
            on_accent: false,
        });
        let text_x = MARGIN + GLYPH + GLYPH_GAP;
        let text_width = WIDTH - MARGIN - text_x;
        let mut heading =
            label(text_x, top, text_width, self.title, 20.0, 28.0, tone(TextTone::Primary, theme));
        heading.weight = 600;
        b.text(&content.title, heading);
        if self.message > 0.0 {
            let y = top + self.title + 4.0;
            b.text(
                &content.message,
                label(
                    text_x,
                    y,
                    text_width,
                    self.message,
                    14.0,
                    20.0,
                    tone(TextTone::Secondary, theme),
                ),
            );
        }
        let mut y = top + self.header();
        if !self.summary.is_empty() {
            y += SPACING;
            let card = rect(MARGIN, y, WIDTH - 2.0 * MARGIN, self.summary_height());
            b.shell.push(Element::Decoration { bounds: card, appearance: Decoration::Card });
            let mut row_y = y + 10.0;
            let value_x = MARGIN + 12.0 + self.summary_label + 18.0;
            for (row, (label_height, value_height)) in content.summary.iter().zip(&self.summary) {
                b.text(
                    &row.label,
                    label(
                        MARGIN + 12.0,
                        row_y + 2.0,
                        self.summary_label.max(1.0),
                        *label_height,
                        12.0,
                        16.0,
                        tone(TextTone::Secondary, theme),
                    ),
                );
                let mut value = label(
                    value_x,
                    row_y,
                    WIDTH - MARGIN - 12.0 - value_x,
                    *value_height,
                    14.0,
                    20.0,
                    tone(TextTone::Primary, theme),
                );
                value.weight = 600;
                b.text(&row.value, value);
                row_y += label_height.max(*value_height) + 6.0;
            }
            y += self.summary_height();
        }
        let mut page = Vec::new();
        let mut viewport = rect(MARGIN, y, WIDTH - 2.0 * MARGIN, 1.0);
        let mut content_height = 1.0;
        let mut scroll = 0.0;
        if self.details > 0.0 {
            y += SPACING;
            let toggle = rect(MARGIN, y, WIDTH - 2.0 * MARGIN, BUTTON);
            b.button(
                ReportAction::ToggleDetails,
                toggle,
                false,
                labels.action(ReportAction::ToggleDetails, state.expanded),
            );
            y += BUTTON;
            let panel = self.details_panel(state.expansion);
            if panel.height >= 2.0 {
                b.shell.push(Element::Decoration {
                    bounds: panel,
                    appearance: Decoration::Field {
                        selection: false,
                        focused: false,
                        state: ControlState::Rest,
                    },
                });
                viewport =
                    rect(panel.x + 1.0, panel.y + 1.0, panel.width - 2.0, panel.height - 2.0);
                let measured = details.map_or(0.0, DetailsText::height);
                content_height = (measured + 2.0 * DETAILS_PADDING).max(viewport.height);
                scroll = state.scroll.clamp(0.0, content_height - viewport.height);
                if let Some(details) = details {
                    let color = tone(TextTone::Primary, theme);
                    let text_width = Self::details_text_width();
                    let top = scroll - DETAILS_PADDING;
                    for (text, line_y, line_height) in details.visible(top, top + viewport.height) {
                        let mut layout =
                            DetailsText::layout(line_y - top - 1.0, text_width, line_height, color);
                        layout.origin[0] = DETAILS_PADDING - 1.0;
                        if !text.is_empty() {
                            page.push(Element::Label { text: text.to_owned(), layout });
                        }
                    }
                }
            }
            y += (SPACING + self.details) * state.expansion.clamp(0.0, 1.0);
        }
        if let Some((label_height, warning)) = self.consent {
            y += SPACING;
            let check = rect(MARGIN, y + (label_height.min(20.0) - CHECK) * 0.5, CHECK, CHECK);
            let row = rect(MARGIN, y, WIDTH - 2.0 * MARGIN, label_height);
            let checked = state.include_dump;
            let check_state = b.peer_role(ReportAction::IncludeDump, row, Role::Check(checked));
            b.shell.push(Element::Decoration {
                bounds: check,
                appearance: Decoration::Check { checked, state: check_state },
            });
            if checked {
                b.shell.push(Element::Icon {
                    bounds: rect(check.x + 4.0, check.y + 4.0, 10.0, 10.0),
                    icon: FluentIcon::Accept,
                    on_accent: true,
                });
            }
            let x = MARGIN + CHECK + 9.0;
            let w = WIDTH - MARGIN - x;
            b.text(
                labels.include_dump,
                label(x, y, w, label_height, 14.0, 20.0, tone(TextTone::Primary, theme)),
            );
            b.text(
                labels.dump_warning,
                label(
                    x,
                    y + label_height + 4.0,
                    w,
                    warning,
                    12.0,
                    16.0,
                    tone(TextTone::Error, theme),
                ),
            );
        }
        let footer_top = self.body_end(state.expansion) + BOTTOM;
        b.shell.push(Element::Decoration {
            bounds: rect(0.0, footer_top, width, height - footer_top),
            appearance: Decoration::Footer,
        });
        let mut right = WIDTH - MARGIN;
        let button_y = footer_top + (FOOTER - BUTTON) * 0.5;
        for (action, button_width) in self.buttons.iter().rev() {
            right -= button_width;
            let primary = *action == state.primary();
            b.button(
                *action,
                rect(right, button_y, *button_width, BUTTON),
                primary,
                labels.action(*action, false),
            );
            right -= SPACING;
        }
        if let Some(status) = &state.status
            && right - MARGIN > 24.0
        {
            let t = if status.error { TextTone::Error } else { TextTone::Secondary };
            let mut layout =
                label(MARGIN, footer_top + 26.0, right - MARGIN, 32.0, 12.0, 16.0, tone(t, theme));
            layout.clip = Some([MARGIN, footer_top + 26.0, right, footer_top + 58.0]);
            b.text(&status.text, layout);
        }
        Ok(ReportFrame {
            shell: b.shell,
            page,
            controls: b.controls,
            viewport,
            drag: rect(0.0, 0.0, WIDTH - CLOSE_WIDTH, TITLE_BAR),
            content_height,
            scroll,
        })
    }
}

/// Composition scratch for one frame.
struct Builder<'a> {
    theme: Theme,
    labels: &'a ReportLabels,
    state: &'a ReportState,
    shell: Vec<Element>,
    controls: Vec<Control>,
}

impl Builder<'_> {
    fn text(&mut self, text: &str, layout: LabelLayout) {
        if !text.is_empty() {
            self.shell.push(Element::Label { text: text.to_owned(), layout });
        }
    }
    fn peer(&mut self, action: ReportAction, bounds: Bounds) -> ControlState {
        self.peer_role(action, bounds, Role::Button)
    }
    fn peer_role(&mut self, action: ReportAction, bounds: Bounds, role: Role) -> ControlState {
        let command = self.state.command(action);
        let mut control = peer(action, self.labels, self.state, bounds, command);
        control.role = role;
        self.controls.push(control);
        visual_state(self.state, action, command)
    }
    fn button(&mut self, action: ReportAction, bounds: Bounds, primary: bool, text: &str) {
        let state = self.peer(action, bounds);
        self.shell.push(Element::Decoration {
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
