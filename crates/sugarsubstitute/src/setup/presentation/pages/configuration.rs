// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Managed environment and attached-runtime forms retain upstream labels and user paths exactly.
use super::*;
use sugarsubstitute_ui::form::Role;

impl Builder<'_> {
    /// Arrange managed provisioning options or attached runtime paths from actual flow state.
    pub(in super::super) fn configuration_page(
        &mut self,
        x: f32,
        y: f32,
        width: f32,
        compact: bool,
    ) -> io::Result<f32> {
        let managed = self.state.mode == RuntimeMode::Managed;
        let hero = self.hero(
            Some(if managed { FluentIcon::Home } else { FluentIcon::Link }),
            if managed {
                Text::ConfigurationTitleManaged
            } else {
                Text::ConfigurationTitleAttached
            },
            Some(if managed {
                Text::ConfigurationDescriptionManaged
            } else {
                Text::ConfigurationDescriptionAttached
            }),
            bounds(x, y, width, 8192.0),
            false,
            false,
        )?;
        let top = y + hero + 18.0;
        let index = self.elements.len();
        let inner = width - 38.0;
        let left = x + 19.0;
        let mut next = top + 17.0;
        if managed {
            let column = if compact { inner } else { (inner - 18.0) * 0.5 };
            let release =
                self.option_field(OptionField::Release, Text::Channel, left, next, column)?;
            let version_y = if compact { next + release + 18.0 } else { next };
            let version_x = if compact { left } else { left + column + 18.0 };
            let version = self.option_field(
                OptionField::ComfyVersion,
                Text::Version,
                version_x,
                version_y,
                column,
            )?;
            next = if compact { version_y + version } else { next + release.max(version) } + 14.0;
            next +=
                self.option_field(OptionField::Variant, Text::Hardware, left, next, inner)? + 14.0;
            let detail = self.state.variant_description.as_str();
            next += self.label(
                detail,
                bounds(left, next, inner, 8192.0),
                12.0,
                18.0,
                TextTone::Secondary,
            )? + 14.0;
            let label_height =
                self.measure(self.copy.mirrors, inner - 27.0, 14.0, 19.0)?.height.max(19.0);
            let check = bounds(left, next + (label_height - 18.0) * 0.5, 18.0, 18.0);
            self.decoration(check, Decoration::Check { checked: self.state.mirrors });
            if self.state.mirrors {
                self.icon(bounds(check.x + 4.0, check.y + 4.0, 10.0, 10.0), FluentIcon::Accept);
            }
            self.label(
                self.copy.mirrors,
                bounds(left + 27.0, next, inner - 27.0, label_height),
                14.0,
                19.0,
                TextTone::Primary,
            )?;
            self.peer(
                Action::Mirrors,
                self.copy.mirrors,
                bounds(left, next, inner, label_height),
                Role::Check(self.state.mirrors),
                self.state.can_edit,
            );
            next += label_height;
        } else {
            next += self.path(
                PathField::Comfy,
                Text::ComfyFolder,
                bounds(left, next, inner, 8192.0),
                &self.state.paths.comfy,
                33.0,
            )? + 14.0;
            next += self.path(
                PathField::Python,
                Text::Python,
                bounds(left, next, inner, 8192.0),
                &self.state.paths.python,
                33.0,
            )? + 8.0;
            next += self.label(
                self.copy.python_help,
                bounds(left, next, inner, 8192.0),
                12.0,
                18.0,
                TextTone::Secondary,
            )?;
        }
        next += 17.0;
        self.elements.insert(
            index,
            Element::Decoration {
                bounds: bounds(x, top, width, next - top),
                appearance: Decoration::Section,
            },
        );
        Ok(next)
    }

    /// Keep upstream labels and selection identities with their measured localized heading.
    fn option_field(
        &mut self,
        field: OptionField,
        key: Text,
        x: f32,
        y: f32,
        width: f32,
    ) -> io::Result<f32> {
        let label = key.text(self.copy).to_uppercase();
        let height =
            self.label(&label, bounds(x, y, width, 8192.0), 12.0, 16.8, TextTone::Primary)?;
        let labels = self.state.options(field).labels.clone();
        self.select(
            Action::Option(field),
            key,
            bounds(x, y + height + 8.0, width, 33.0),
            labels,
            self.state.options(field).selected,
        )?;
        Ok(height + 8.0 + 33.0)
    }
}
