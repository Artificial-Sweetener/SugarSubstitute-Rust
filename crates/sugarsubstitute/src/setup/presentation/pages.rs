// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Measured installer page layout. Shared widgets own all drawing and native-control semantics.
mod configuration;
mod progress;
mod runtime;
use super::{builder::bounds, *};
use sugarsubstitute_contracts::localization::LOCALES;
use sugarsubstitute_ui::{
    icons::FluentIcon,
    widgets::{Decoration, TextTone},
};

impl Builder<'_> {
    /// Compose one application page and return its full logical height before centering or scroll.
    pub(super) fn page(&mut self, width: f32, available_height: f32) -> io::Result<f32> {
        let page = self.state.page;
        let expanded = page == Page::Progress && self.state.details_open;
        let maximum: f32 = match page {
            Page::Language => 620.0,
            Page::Location => 1030.0,
            Page::Configuration | Page::Complete => 820.0,
            Page::Folders => 720.0,
            Page::Progress if !expanded => 760.0,
            _ => 980.0,
        };
        let page_width = width.min(maximum);
        let large = matches!(page, Page::Language | Page::Location);
        let compact = self.view.surface.width as f32 / self.view.surface.scale <= 740.0;
        let padding = if large && !compact { 24.0 } else { 4.0 };
        let x = (width - page_width) * 0.5 + padding;
        let inner_width = page_width - 2.0 * padding;
        let y = if large { 20.0 } else { 6.0 };
        let end = match page {
            Page::Language => {
                let hero_width = inner_width.min(484.0);
                let hero = self.hero(
                    Some(FluentIcon::Language),
                    Text::LanguageTitle,
                    Some(Text::LanguageDescription),
                    bounds(x + (inner_width - hero_width) * 0.5, y, hero_width, 8192.0),
                    true,
                    false,
                )?;
                let field = bounds(
                    x + (inner_width - inner_width.min(420.0)) * 0.5,
                    y + hero + 24.0,
                    inner_width.min(420.0),
                    33.0,
                );
                let selected =
                    LOCALES.iter().position(|locale| locale.id == self.state.selected_locale);
                self.select(
                    Action::Locale,
                    Text::LanguageTitle,
                    field,
                    LOCALES.iter().map(|locale| locale.name.to_owned()).collect(),
                    selected,
                )?;
                field.y + field.height
            }
            Page::Location => {
                let hero = self.hero(
                    Some(FluentIcon::Folder),
                    Text::LocationTitle,
                    Some(Text::LocationDescription),
                    bounds(x, y, inner_width, 8192.0),
                    true,
                    false,
                )?;
                let field_y = y + hero + 28.0;
                field_y
                    + self.path(
                        PathField::Installation,
                        Text::InstallFolder,
                        bounds(x, field_y, inner_width, 8192.0),
                        &self.state.paths.installation,
                        38.0,
                    )?
            }
            Page::Runtime => self.runtime_page(x, y, inner_width, compact)?,
            Page::Configuration => self.configuration_page(x, y, inner_width, compact)?,
            Page::Folders => self.folders_page(x, y, inner_width)?,
            Page::Maintenance => self.maintenance_page(x, y, inner_width)?,
            Page::Complete => self.complete_page(x, y, inner_width)?,
            Page::Progress => {
                self.progress_page(x, y, inner_width, available_height - 8.0, expanded)?
            }
        };
        Ok(end + if large { 20.0 } else { 8.0 })
    }

    /// Present shared model and output locations with the original preservation explanation.
    fn folders_page(&mut self, x: f32, y: f32, width: f32) -> io::Result<f32> {
        let hero = self.hero(
            Some(FluentIcon::Folder),
            Text::FoldersTitle,
            None,
            bounds(x, y, width, 8192.0),
            false,
            true,
        )?;
        let top = y + hero + 18.0;
        let index = self.elements.len();
        let mut next = top + 17.0;
        for (field, key, value) in [
            (PathField::Models, Text::Models, self.state.paths.models.as_str()),
            (PathField::Outputs, Text::Outputs, self.state.paths.outputs.as_str()),
        ] {
            next +=
                self.path(field, key, bounds(x + 19.0, next, width - 38.0, 8192.0), value, 33.0)?
                    + 14.0;
        }
        next += self.label(
            self.copy.preservation,
            bounds(x + 19.0, next, width - 38.0, 8192.0),
            12.0,
            18.0,
            TextTone::Secondary,
        )? + 17.0;
        self.elements.insert(
            index,
            Element::Decoration {
                bounds: bounds(x, top, width, next - top),
                appearance: Decoration::Section,
            },
        );
        Ok(next)
    }

    /// Show verified existing-installation facts and the appropriate recovery explanation.
    fn maintenance_page(&mut self, x: f32, y: f32, width: f32) -> io::Result<f32> {
        let existing = self
            .state
            .existing
            .as_ref()
            .ok_or_else(|| io::Error::other("Maintenance facts absent"))?;
        let notice = if existing.entry_missing {
            Text::MaintenanceLaunchMissing
        } else if existing.executable_missing {
            Text::MaintenanceMissing
        } else {
            Text::MaintenanceDescription
        };
        let hero = self.hero(
            Some(FluentIcon::Sync),
            Text::MaintenanceTitle,
            Some(notice),
            bounds(x, y, width, 8192.0),
            false,
            false,
        )?;
        self.summary(
            x,
            y + hero + 18.0,
            width,
            &[
                (Text::InstallFolder, &existing.install_root),
                (Text::ComfyFolder, &existing.comfy_root),
            ],
        )
    }

    /// Present the committed installation location using the same summary treatment as repair.
    fn complete_page(&mut self, x: f32, y: f32, width: f32) -> io::Result<f32> {
        let hero = self.hero(
            Some(FluentIcon::Accept),
            Text::CompleteTitle,
            None,
            bounds(x, y, width, 8192.0),
            false,
            true,
        )?;
        let path = self
            .state
            .existing
            .as_ref()
            .map_or(self.state.paths.installation.as_str(), |existing| {
                existing.install_root.as_str()
            });
        self.summary(x, y + hero + 18.0, width, &[(Text::InstallFolder, path)])
    }

    /// Measure immutable user paths without truncation or changing their Unicode content.
    fn summary(&mut self, x: f32, y: f32, width: f32, fields: &[(Text, &str)]) -> io::Result<f32> {
        let index = self.elements.len();
        let mut next = y + 20.0;
        for (i, (key, value)) in fields.iter().enumerate() {
            if i > 0 {
                next += 8.0;
            }
            next += self.label(
                key.text(self.copy),
                bounds(x + 20.0, next, width - 40.0, 8192.0),
                12.0,
                18.0,
                TextTone::Secondary,
            )? + 8.0;
            next += self.label(
                value,
                bounds(x + 20.0, next, width - 40.0, 8192.0),
                14.0,
                19.0,
                TextTone::Primary,
            )?;
        }
        next += 20.0;
        self.elements.insert(
            index,
            Element::Decoration {
                bounds: bounds(x, y, width, next - y),
                appearance: Decoration::Summary,
            },
        );
        Ok(next)
    }
}
