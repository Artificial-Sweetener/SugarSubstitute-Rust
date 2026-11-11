// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Compose one measured report into scene commands and its accessible peers.
use super::body::{Builder, Lines};
use super::layout::*;
use super::{DetailsText, ReportAction, ReportContent, ReportLabels, ReportState};
use crate::{
    accent::AccentPalette,
    form::Control,
    icons::FluentIcon,
    scene::{Element, FramePaint, Layer},
    text::PaintSpec,
    widgets::{Bounds, ControlState, Decoration, TextTone},
};
use std::io;
use sugarsubstitute_contracts::{execution::MemoryLease, presentation::FrameIdentity};

/// Scene commands and native peers for exactly one prepared frame.
pub struct ReportFrame {
    /// Whole-window layer.
    pub shell: Vec<Element>,
    /// Scrolled layer, relative to `viewport` and already displaced by `scroll`.
    pub page: Vec<Element>,
    /// Window-relative peers for this frame.
    pub controls: Vec<Control>,
    /// Peers inside the scrolled layer, relative to `viewport` and already displaced.
    pub page_controls: Vec<Control>,
    /// Logical scrolled viewport inside the window: the details text when the window fits the
    /// report, otherwise the whole body between the title bar and the footer.
    pub viewport: Bounds,
    /// Logical caption drag region.
    pub drag: Bounds,
    /// Full scrolled content height, for the host scrollbar.
    pub content_height: f32,
    /// Clamped scroll offset.
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

    fn empty() -> Self {
        Self {
            shell: Vec::new(),
            page: Vec::new(),
            controls: Vec::new(),
            page_controls: Vec::new(),
            viewport: rect(MARGIN, TITLE_BAR, WIDTH - 2.0 * MARGIN, 1.0),
            drag: rect(0.0, 0.0, WIDTH - CLOSE_WIDTH, TITLE_BAR),
            content_height: 1.0,
            scroll: 0.0,
        }
    }
}

impl ReportLayout {
    /// Compose one frame at `surface`. When the surface is at least the report's natural
    /// height the details text scrolls inside its panel; when it is shorter, the title bar and
    /// footer stay in place and the body between them scrolls instead. `details` supplies the
    /// measured lines; lines not measured yet are not painted.
    /// # Errors
    /// Rejects a surface narrower than the report.
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
        if width + 0.5 < WIDTH {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, "Report surface too narrow"));
        }
        let expansion = state.expansion.clamp(0.0, 1.0);
        let mut shell = Builder::new(surface.theme, labels, state);
        Self::title_bar(&mut shell, content);
        let mut body = Builder::new(surface.theme, labels, state);
        let mut frame = ReportFrame::empty();
        let footer = if height + 0.5 >= self.height(expansion) {
            let opening = self.opening(self.details_height(), expansion);
            let lines = Lines::Panel { scroll: state.scroll };
            if let Some(panel) = self.body(&mut body, content, details, opening, lines) {
                frame.page = panel.elements;
                frame.viewport = panel.viewport;
                frame.content_height = panel.content_height;
                frame.scroll = panel.scroll;
            }
            shell.elements.append(&mut body.elements);
            shell.controls.append(&mut body.controls);
            self.body_end(self.details_height(), expansion) + BOTTOM
        } else {
            let footer = (height - FOOTER).max(TITLE_BAR + 1.0);
            self.scrolled_body(&mut frame, &mut body, content, details, state, footer);
            footer
        };
        self.footer(&mut shell, state, footer, width, height);
        frame.shell = shell.elements;
        frame.controls = shell.controls;
        Ok(frame)
    }

    /// The body as one scrolled layer between the title bar and the footer at `footer`.
    fn scrolled_body(
        &self,
        frame: &mut ReportFrame,
        body: &mut Builder<'_>,
        content: &ReportContent,
        details: Option<&DetailsText>,
        state: &ReportState,
        footer: f32,
    ) {
        let expansion = state.expansion.clamp(0.0, 1.0);
        frame.viewport = rect(0.0, TITLE_BAR, WIDTH, footer - TITLE_BAR);
        let full = details.map_or(0.0, DetailsText::height) + 2.0 * DETAILS_PADDING;
        let natural = self.body_end(full, expansion) + BOTTOM - TITLE_BAR;
        frame.content_height = natural.max(frame.viewport.height);
        frame.scroll = state.scroll.clamp(0.0, frame.content_height - frame.viewport.height);
        let top = TITLE_BAR + frame.scroll;
        let bottom = top + frame.viewport.height;
        let opening = self.opening(full, expansion);
        self.body(body, content, details, opening, Lines::Body { top, bottom });
        frame.page = std::mem::take(&mut body.elements)
            .into_iter()
            .filter(|element| overlaps(element, top, bottom))
            .map(|mut element| {
                element.translate(0.0, -top);
                element
            })
            .collect();
        frame.page_controls = std::mem::take(&mut body.controls)
            .into_iter()
            .map(|mut control| {
                if control.bounds.y + control.bounds.height < top || control.bounds.y > bottom {
                    // Scrolled far out of view: hidden, with geometry the native peer accepts.
                    control.availability.visible = false;
                    control.bounds = rect(0.0, 0.0, 1.0, 1.0);
                } else {
                    control.bounds.y -= top;
                }
                control
            })
            .collect();
    }

    /// Caption title and close; the rest of the strip moves the window.
    fn title_bar(b: &mut Builder<'_>, content: &ReportContent) {
        let primary = tone(TextTone::Primary, b.theme);
        let caption = label(16.0, 9.0, WIDTH - CLOSE_WIDTH - 32.0, 16.0, 12.0, 16.0, primary);
        b.text(&content.window_title, caption);
        let close = rect(WIDTH - CLOSE_WIDTH, 0.0, CLOSE_WIDTH, TITLE_BAR);
        let dismiss = b.peer(ReportAction::Dismiss, close);
        b.elements.push(Element::Decoration {
            bounds: close,
            appearance: Decoration::Close { state: dismiss },
        });
        let glyph = if dismiss == ControlState::Rest { primary } else { [255; 4] };
        b.elements.push(Element::Glyph {
            bounds: rect(WIDTH - 28.0, 11.0, 10.0, 10.0),
            icon: FluentIcon::Close,
            color: glyph,
        });
    }

    /// Command band: right-aligned commands, the status line to their left.
    fn footer(&self, b: &mut Builder<'_>, state: &ReportState, top: f32, width: f32, height: f32) {
        b.elements.push(Element::Decoration {
            bounds: rect(0.0, top, width, (height - top).max(1.0)),
            appearance: Decoration::Footer,
        });
        let mut right = WIDTH - MARGIN;
        let button_y = top + (FOOTER - BUTTON) * 0.5;
        for (action, button_width) in self.buttons.iter().rev() {
            right -= button_width;
            let primary = *action == state.primary();
            let text = b.labels.action(*action, false);
            b.button(*action, rect(right, button_y, *button_width, BUTTON), primary, text);
            right -= SPACING;
        }
        if let Some(status) = &state.status
            && right - MARGIN > 24.0
        {
            let t = if status.error { TextTone::Error } else { TextTone::Secondary };
            let mut layout =
                label(MARGIN, top + 26.0, right - MARGIN, 32.0, 12.0, 16.0, tone(t, b.theme));
            layout.clip = Some([MARGIN, top + 26.0, right, top + 58.0]);
            b.text(&status.text, layout);
        }
    }
}

/// Whether an element's vertical extent reaches into the band `[top, bottom)`.
fn overlaps(element: &Element, top: f32, bottom: f32) -> bool {
    let (y, height) = match element {
        Element::Decoration { bounds, .. }
        | Element::Icon { bounds, .. }
        | Element::Glyph { bounds, .. }
        | Element::Artwork { bounds, .. } => (bounds.y, bounds.height),
        Element::Label { layout, .. } => (layout.origin[1], layout.extent[1]),
        Element::Progress { bar, .. } => (bar.y, bar.height),
    };
    y < bottom && y + height > top
}
