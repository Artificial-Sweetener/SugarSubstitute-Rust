// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Installer composition helpers; drawing, font shaping and native input remain shared owners.
use super::*;
use sugarsubstitute_ui::{
    availability::Availability,
    form::Role,
    icons::FluentIcon,
    text::{LabelLayout, LabelPaintSpec, TextAlignment, TextMetrics, VerticalAnchor},
    widgets::{ControlState, Decoration, TextTone},
};

/// One admitted page preparation, borrowing its application snapshot and retained font owner.
pub(super) struct Builder<'a> {
    pub(super) state: &'a Snapshot,
    pub(super) view: View,
    pub(super) resources: &'a mut SceneResources,
    pub(super) lease: Option<&'a MemoryLease>,
    pub(super) copy: Copy,
    pub(super) elements: Vec<Element>,
    pub(super) controls: Vec<Control>,
    pub(super) console: Option<Bounds>,
}

/// Keep page placement in logical coordinates until the shared renderer applies display scale.
pub(super) fn bounds(x: f32, y: f32, width: f32, height: f32) -> Bounds {
    Bounds { x, y, width, height }
}

impl Builder<'_> {
    /// Apply measured page centering/scrolling to pixels and their original native hit regions.
    pub(super) fn translate(&mut self, delta: f32) {
        for element in &mut self.elements {
            element.translate(0.0, delta);
        }
        for peer in &mut self.controls {
            peer.bounds.y += delta;
        }
        if let Some(console) = &mut self.console {
            console.y += delta;
        }
    }
    /// Submit a shared treatment without introducing page-specific painting rules.
    pub(super) fn decoration(&mut self, bounds: Bounds, appearance: Decoration) {
        self.elements.push(Element::Decoration { bounds, appearance });
    }
    /// Retain the canonical Fluent asset identity and its centered viewport.
    pub(super) fn icon(&mut self, bounds: Bounds, icon: FluentIcon) {
        self.elements.push(Element::Icon { bounds, icon, on_accent: false });
    }
    /// Resolve actual native observations with application availability taking precedence.
    pub(super) fn state(&self, action: Action, enabled: bool) -> ControlState {
        if !enabled {
            ControlState::Disabled
        } else if self.view.interaction.pressed == Some(action.id()) {
            ControlState::Pressed
        } else if self.view.interaction.hovered == Some(action.id()) {
            ControlState::Hover
        } else {
            ControlState::Rest
        }
    }
    /// Couple painted geometry to the accessible native control that owns its input.
    pub(super) fn peer(
        &mut self,
        action: Action,
        label: &'static str,
        bounds: Bounds,
        role: Role,
        enabled: bool,
    ) {
        self.controls.push(Control {
            id: action.id(),
            label,
            bounds,
            availability: Availability { visible: true, enabled },
            text_revision: action.text_slot().map_or(0, |slot| self.view.text_revisions[slot]),
            role,
        });
    }
    /// Define natural typography for measurement and painting through the same font owner.
    pub(super) fn layout(&self, rect: Bounds, size: f32, line: f32, tone: TextTone) -> LabelLayout {
        LabelLayout {
            origin: [rect.x, rect.y],
            extent: [rect.width, rect.height],
            clip: None,
            anchor: VerticalAnchor::Top,
            alignment: TextAlignment::Leading,
            metrics: TextMetrics::Natural,
            font_size: size,
            line_height: line,
            weight: 400,
            color: tone.color(self.view.surface.theme),
        }
    }
    /// Measure complete wrapped text before applying the page's visible clipping rectangle.
    pub(super) fn measure(
        &mut self,
        text: &str,
        width: f32,
        size: f32,
        line: f32,
    ) -> io::Result<sugarsubstitute_ui::text::TextSize> {
        let layout = self.layout(bounds(0.0, 0.0, width, 8192.0), size, line, TextTone::Primary);
        self.resources.measure(
            text,
            LabelPaintSpec { surface: self.view.surface, layout },
            self.lease,
        )
    }
    /// Place one label using measured line count; user and upstream text is never rewritten.
    pub(super) fn label(
        &mut self,
        text: &str,
        rect: Bounds,
        size: f32,
        line: f32,
        tone: TextTone,
    ) -> io::Result<f32> {
        if text.is_empty() {
            return Ok(0.0);
        }
        let height = self.measure(text, rect.width, size, line)?.height;
        let mut layout =
            self.layout(bounds(rect.x, rect.y, rect.width, height.max(line)), size, line, tone);
        if rect.height < height {
            layout.clip = Some([rect.x, rect.y, rect.x + rect.width, rect.y + rect.height]);
        }
        self.elements.push(Element::Label { text: text.to_owned(), layout });
        Ok(height.min(rect.height))
    }
    /// Preserve intrinsic localized button width, padding and the surface's minimum width.
    pub(super) fn button_width(&mut self, key: Text, minimum: f32) -> io::Result<f32> {
        Ok((self.measure(key.text(self.copy), 4096.0, 14.0, 19.0)?.width + 26.0).max(minimum))
    }
    /// Compose button chrome and centered text with one authoritative native action peer.
    pub(super) fn button(
        &mut self,
        action: Action,
        key: Text,
        rect: Bounds,
        primary: bool,
        enabled: bool,
    ) {
        let state = self.state(action, enabled);
        self.decoration(rect, Decoration::Button { primary, state });
        let mut layout = self.layout(
            bounds(rect.x + 12.0, rect.y + 6.0, rect.width - 24.0, 19.0),
            14.0,
            19.0,
            TextTone::Action { primary, state },
        );
        layout.alignment = TextAlignment::Center;
        self.elements.push(Element::Label { text: key.text(self.copy).into(), layout });
        self.peer(action, key.text(self.copy), rect, Role::Button, enabled);
    }
    /// Compose the closed selection while the native peer owns keyboard and popup interaction.
    pub(super) fn select(
        &mut self,
        action: Action,
        key: Text,
        rect: Bounds,
        labels: Vec<String>,
        selected: Option<usize>,
    ) -> io::Result<()> {
        let enabled = self.state.can_edit && !labels.is_empty();
        let state = self.state(action, enabled);
        self.decoration(rect, Decoration::Field { selection: true, focused: false, state });
        let text = selected.and_then(|i| labels.get(i)).map_or("", String::as_str);
        self.label(
            text,
            bounds(rect.x + 12.0, rect.y + 6.0, rect.width - 44.0, 19.0),
            14.0,
            19.0,
            TextTone::Primary,
        )?;
        self.icon(
            bounds(rect.x + rect.width - 22.0, rect.y + (rect.height - 10.0) * 0.5, 10.0, 10.0),
            FluentIcon::ChevronDown,
        );
        self.peer(action, key.text(self.copy), rect, Role::Select { labels, selected }, enabled);
        Ok(())
    }
    /// Pair a native editable path with its shell-picker action and measured field label.
    pub(super) fn path(
        &mut self,
        field: PathField,
        key: Text,
        rect: Bounds,
        value: &str,
        input_height: f32,
    ) -> io::Result<f32> {
        let label = key.text(self.copy).to_uppercase();
        let label_height = self.label(
            &label,
            bounds(rect.x, rect.y, rect.width, 8192.0),
            12.0,
            16.8,
            TextTone::Primary,
        )?;
        let y = rect.y + label_height + 8.0;
        let browse = self.button_width(Text::Browse, 0.0)?;
        let input = bounds(rect.x, y, rect.width - browse - 10.0, input_height);
        let action = Action::Path(field);
        self.decoration(
            input,
            Decoration::Field {
                selection: false,
                focused: self.view.interaction.focused == Some(action.id()),
                state: self.state(action, self.state.can_edit),
            },
        );
        // The actual native text peer owns every field glyph, selection and caret.
        self.peer(
            action,
            key.text(self.copy),
            input,
            Role::TextField(value.into()),
            self.state.can_edit,
        );
        self.button(
            Action::Browse(field),
            Text::Browse,
            bounds(input.x + input.width + 10.0, y + (input_height - 32.0) * 0.5, browse, 32.0),
            false,
            self.state.can_edit,
        );
        Ok(label_height + 8.0 + input_height)
    }
    /// Arrange the original heading, supporting text and centered Fluent badge.
    pub(super) fn hero(
        &mut self,
        icon: Option<FluentIcon>,
        title: Text,
        description: Option<Text>,
        rect: Bounds,
        large: bool,
        compact: bool,
    ) -> io::Result<f32> {
        let badge = if large { 48.0 } else { 44.0 };
        let gap = if large { 16.0 } else { 14.0 };
        let text_x = rect.x + if icon.is_some() { badge + gap } else { 0.0 };
        let text_width = rect.width - (text_x - rect.x);
        let size = if large { 20.0 } else { 22.0 };
        let title_height =
            self.measure(title.text(self.copy), text_width, size, size * 1.3)?.height;
        let y = rect.y + if compact { (badge - title_height) * 0.5 } else { 1.0 };
        self.label(
            title.text(self.copy),
            bounds(text_x, y, text_width, 8192.0),
            size,
            size * 1.3,
            TextTone::Primary,
        )?;
        let mut height = if compact { badge.max(title_height) } else { 1.0 + title_height + 5.0 };
        if let Some(key) = description {
            height += self.label(
                key.text(self.copy),
                bounds(text_x, rect.y + height, text_width, 8192.0),
                if large { 14.0 } else { 12.0 },
                if large { 21.0 } else { 18.0 },
                if large { TextTone::Primary } else { TextTone::Secondary },
            )?;
        }
        if let Some(icon) = icon {
            self.decoration(bounds(rect.x, rect.y, badge, badge), Decoration::IconBadge);
            let glyph = if large { 24.0 } else { 22.0 };
            let inset = (badge - glyph) * 0.5;
            self.icon(bounds(rect.x + inset, rect.y + inset, glyph, glyph), icon);
            height = height.max(badge);
        }
        Ok(height)
    }
}
