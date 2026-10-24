// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Installer branding, journey, feedback and footer layout surrounding the clipped page viewport.
use super::{builder::bounds, *};
use sugarsubstitute_application::setup_flow::Failure;
use sugarsubstitute_ui::{
    form::Role,
    icons::FluentIcon,
    progress::{ProgressAppearance, ProgressBar, ProgressFrame},
    text::TextAlignment,
    widgets::{Decoration, TextTone},
};

pub(super) const BRAND_HEIGHT: f32 = 126.0;

impl Builder<'_> {
    /// Reserve measured feedback and navigation space before returning the page viewport.
    pub(super) fn shell(&mut self, width: f32, height: f32) -> io::Result<Bounds> {
        let compact = width <= 740.0;
        let horizontal = if compact { 20.0 } else { 44.0 };
        let top = BRAND_HEIGHT + 1.0 + if compact { 20.0 } else { 26.0 };
        let bottom = height - if compact { 20.0 } else { 24.0 };
        self.decoration(bounds(0.0, BRAND_HEIGHT, width, height - BRAND_HEIGHT), Decoration::Body);
        self.elements.push(Element::Artwork {
            bounds: bounds(38.0, 21.0, 270.0, 88.0),
            source: include_str!("../../../../../assets/brand/wordmark.svg"),
        });
        let (current, total) = self.state.journey;
        let step = self
            .copy
            .step
            .replace("{current}", &current.to_string())
            .replace("{total}", &total.to_string());
        let page = match self.state.page {
            Page::Language => Text::StepLanguage,
            Page::Location => Text::StepLocation,
            Page::Runtime => Text::StepRuntime,
            Page::Configuration => Text::StepConfiguration,
            Page::Folders => Text::StepFolders,
            Page::Maintenance => Text::StepMaintenance,
            Page::Progress => Text::StepProgress,
            Page::Complete => Text::StepComplete,
        };
        let step = format!("{step} · {}", page.text(self.copy));
        let journey_width = self.measure(&step, 4096.0, 12.0, 16.0)?.width.max(220.0);
        let journey_x = width - 52.0 - journey_width;
        let mut layout = self.layout(
            bounds(journey_x, 50.5, journey_width, 16.0),
            12.0,
            16.0,
            TextTone::Tertiary,
        );
        layout.alignment = TextAlignment::Trailing;
        self.elements.push(Element::Label { text: step, layout });
        self.progress(bounds(journey_x, 71.5, journey_width, 4.0), current as f32 / total as f32);
        let close = bounds(width - 46.0, 0.0, 46.0, 32.0);
        self.decoration(
            close,
            Decoration::Close { state: self.state(Action::Close, !self.state.closing) },
        );
        self.icon(bounds(width - 28.0, 11.0, 10.0, 10.0), FluentIcon::Close);
        self.peer(Action::Close, self.copy.close, close, Role::Button, !self.state.closing);

        let primary = self.state.primary_key;
        let back = primary.is_some()
            && !matches!(self.state.page, Page::Language | Page::Progress | Page::Complete);
        let footer_height = if primary.is_some() || back { 32.0 } else { 0.0 };
        let mut right = width - horizontal;
        if let Some(key) = primary {
            let size = self.button_width(key, 168.0)?;
            right -= size;
            self.button(
                Action::Primary,
                key,
                bounds(right, bottom - footer_height, size, footer_height),
                true,
                self.state.can_primary,
            );
            right -= 10.0;
        }
        if back {
            let size = self.button_width(Text::Back, 92.0)?;
            self.button(
                Action::Back,
                Text::Back,
                bounds(right - size, bottom - footer_height, size, footer_height),
                false,
                self.state.can_back,
            );
        }
        let mut end = bottom - footer_height - 16.0;
        let content_width = width - 2.0 * horizontal;
        if let Some(failure) = self.state.failure.as_ref() {
            let message = match failure {
                Failure::Message(text) => text.as_str(),
                Failure::Localized(key) => key.text(self.copy),
            };
            let h = self.measure(message, content_width, 12.0, 18.0)?.height.min(72.0);
            end -= h;
            self.label(
                message,
                bounds(horizontal, end, content_width, h),
                12.0,
                18.0,
                TextTone::Error,
            )?;
            end -= 16.0;
        }
        if self.state.waiting {
            let message = self.copy.long_wait;
            let h = self.measure(message, content_width, 12.0, 18.0)?.height;
            end -= h;
            self.label(
                message,
                bounds(horizontal, end, content_width, h),
                12.0,
                18.0,
                TextTone::Secondary,
            )?;
            end -= 16.0;
        }
        if end <= top {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Installer feedback exceeds the viewport",
            ));
        }
        Ok(bounds(horizontal, top, content_width, end - top))
    }

    pub(super) fn progress(&mut self, rect: Bounds, completion: f32) {
        self.elements.push(Element::Progress {
            bar: ProgressBar {
                x: rect.x,
                y: rect.y,
                width: rect.width,
                height: rect.height,
                appearance: ProgressAppearance::Wizard,
            },
            frame: ProgressFrame { completed: completion, activity: None },
        });
    }
}
