// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Progress and native output placement share one measured installation page.
use super::*;
use sugarsubstitute_application::setup_flow::Status;

impl Builder<'_> {
    /// Allocate remaining page height to the shared native transcript when details are revealed.
    pub(in super::super) fn progress_page(
        &mut self,
        x: f32,
        y: f32,
        width: f32,
        bottom: f32,
        expanded: bool,
    ) -> io::Result<f32> {
        let hero = self.hero(
            Some(FluentIcon::Sync),
            Text::ProgressTitle,
            Some(Text::ProgressDescription),
            bounds(x, y, width, 8192.0),
            false,
            false,
        )?;
        let top = y + hero + 18.0;
        let index = self.elements.len();
        let left = x + 23.0;
        let inner = width - 46.0;
        let mut next = top + 23.0;
        let status = match &self.state.status {
            Status::Preparing => self.copy.preparing_installation,
            Status::Message(text) => text.as_str(),
            Status::Cancelling => self.copy.cancelling,
        };
        next +=
            self.label(status, bounds(left, next, inner, 93.6), 24.0, 31.2, TextTone::Primary)?
                + 14.0;
        self.progress(bounds(left, next, inner, 6.0), self.state.overall as f32 / 100.0);
        next += 20.0;
        let key = if expanded { Text::HideDetails } else { Text::Details };
        let button_width = self.button_width(key, 0.0)?;
        self.button(Action::Details, key, bounds(left, next, button_width, 32.0), false, true);
        next += 32.0;
        if expanded {
            next += 12.0;
            let height = (bottom - 23.0 - next).max(40.0);
            let panel = bounds(left, next, inner, height);
            self.decoration(panel, Decoration::Output);
            self.console = Some(bounds(left + 4.0, next + 4.0, inner - 8.0, height - 8.0));
            next += height;
        }
        next += 23.0;
        self.elements.insert(
            index,
            Element::Decoration {
                bounds: bounds(x, top, width, next - top),
                appearance: Decoration::ProgressSection,
            },
        );
        Ok(next)
    }
}
