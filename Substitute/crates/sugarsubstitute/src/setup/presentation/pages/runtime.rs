// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Runtime ownership choices use a shared measured card row and native radio roles.
use super::*;
use sugarsubstitute_ui::form::Role;

impl Builder<'_> {
    /// Equalize choice-card heights in columns and preserve intrinsic heights in compact rows.
    pub(in super::super) fn runtime_page(
        &mut self,
        x: f32,
        y: f32,
        width: f32,
        compact: bool,
    ) -> io::Result<f32> {
        let hero =
            self.hero(None, Text::RuntimeTitle, None, bounds(x, y, width, 8192.0), false, false)?;
        let top = y + hero + 18.0;
        let index = self.elements.len();
        let column = if compact { width - 38.0 } else { (width - 52.0) * 0.5 };
        let texts = [
            (RuntimeMode::Managed, Text::ManagedTitle, Text::ManagedDescription, FluentIcon::Home),
            (
                RuntimeMode::Attached,
                Text::AttachedTitle,
                Text::AttachedDescription,
                FluentIcon::Link,
            ),
        ];
        let mut heights = [0.0_f32; 2];
        for (i, (_, title, description, _)) in texts.iter().enumerate() {
            let title = self.measure(title.text(self.copy), column - 82.0, 18.0, 23.4)?.height;
            let description =
                self.measure(description.text(self.copy), column - 82.0, 12.0, 18.0)?.height;
            heights[i] = (2.0 + title + 5.0 + description).max(40.0) + 62.0;
        }
        if !compact {
            let height = heights[0].max(heights[1]);
            heights.fill(height);
        }
        let mut end = top + 17.0;
        for (i, (mode, title, description, icon)) in texts.into_iter().enumerate() {
            let left = x + 19.0 + if compact { 0.0 } else { i as f32 * (column + 14.0) };
            let cy = top + 17.0 + if compact && i == 1 { heights[0] + 14.0 } else { 0.0 };
            let rect = bounds(left, cy, column, heights[i]);
            let action = Action::Runtime(mode);
            self.decoration(
                rect,
                Decoration::Choice { state: self.state(action, self.state.can_edit) },
            );
            self.decoration(bounds(left + 16.0, cy + 16.0, 40.0, 40.0), Decoration::IconBadge);
            self.icon(bounds(left + 26.0, cy + 26.0, 20.0, 20.0), icon);
            let title_height = self.label(
                title.text(self.copy),
                bounds(left + 66.0, cy + 18.0, column - 82.0, 8192.0),
                18.0,
                23.4,
                TextTone::Primary,
            )?;
            self.label(
                description.text(self.copy),
                bounds(left + 66.0, cy + 23.0 + title_height, column - 82.0, 8192.0),
                12.0,
                18.0,
                TextTone::Secondary,
            )?;
            let checked = self.state.mode == mode;
            let selection_y = cy + heights[i] - 36.0;
            self.decoration(
                bounds(left + 16.0, selection_y, 20.0, 20.0),
                Decoration::Radio { checked },
            );
            self.label(
                if checked { self.copy.selected } else { self.copy.select },
                bounds(left + 44.0, selection_y + 0.5, column - 60.0, 19.0),
                14.0,
                19.0,
                TextTone::Primary,
            )?;
            self.peer(
                action,
                title.text(self.copy),
                rect,
                Role::Radio(checked),
                self.state.can_edit,
            );
            end = end.max(cy + heights[i]);
        }
        end += 17.0;
        self.elements.insert(
            index,
            Element::Decoration {
                bounds: bounds(x, top, width, end - top),
                appearance: Decoration::Section,
            },
        );
        Ok(end)
    }
}
