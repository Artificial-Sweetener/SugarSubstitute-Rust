// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Route original native observations into application-owned setup actions and picker correlation.
use super::*;
use crate::setup::presentation::{Action, Interaction};
use sugarsubstitute_application::setup_flow::{PathField, Request};
use sugarsubstitute_desktop::installer::native::{Event as NativeEvent, Picker, PickerKind};
use sugarsubstitute_ui::form::{ControlId, Input};

/// Keep the original page and document revision through the shell's modal selection.
pub(super) struct PendingPicker {
    request: Picker,
    field: PathField,
    page: u64,
    text_revision: u64,
}
impl Session {
    /// Consume one queued picker only once; its original result correlation remains retained.
    pub(in super::super) fn picker(&mut self) -> Option<Picker> {
        if !self.picker_queued {
            return None;
        }
        self.picker_queued = false;
        self.picker.as_ref().map(|pending| pending.request)
    }
    /// Admit native events against current flow and original page inventory, never display labels.
    pub(in super::super) fn input(
        &mut self,
        controller: &mut Controller,
        event: NativeEvent,
    ) -> Result<()> {
        if self.navigation.flow.is_closing() {
            return Ok(());
        }
        match event {
            NativeEvent::Interaction { hovered, pressed, focused } => {
                let next = Interaction { hovered, pressed, focused };
                if next != self.view.interaction {
                    self.invalidate()?;
                    self.view.interaction = next;
                }
            }
            NativeEvent::Scroll(position) => {
                if !position.is_finite() {
                    return Err("Invalid native scroll position".into());
                }
                let maximum = self
                    .prepared
                    .as_ref()
                    .map_or(0.0, |p| (p.layout.content_height - p.layout.viewport.height).max(0.0));
                let position = position.clamp(0.0, maximum);
                if position != self.view.scroll {
                    self.invalidate()?;
                    self.view.scroll = position;
                }
            }
            NativeEvent::Picked(request, result) => self.picked(request, result)?,
            NativeEvent::Control(id, input) => {
                let Some(action) = Action::from_id(id) else {
                    return Ok(());
                };
                if action == Action::Close {
                    self.native_close = true;
                    return Ok(());
                }
                if !self.available(id) {
                    return Ok(());
                }
                if let (Action::Path(field), Input::Text { value, revision }) = (action, &input) {
                    self.text(field, value.as_deref(), *revision)?;
                    return Ok(());
                }
                let request = self.action(action, input)?;
                if let Some(request) = request {
                    self.navigation.submit(controller, request)?;
                }
            }
        }
        Ok(())
    }
    /// The retained page may accept later text revisions during repaint, but never after navigation.
    fn available(&self, id: ControlId) -> bool {
        self.page_revision == self.navigation.flow.page_revision()
            && self.prepared.as_ref().is_some_and(|p| {
                p.layout
                    .shell_controls
                    .iter()
                    .chain(&p.layout.page_controls)
                    .any(|control| control.id == id && control.availability.actionable())
            })
    }
    /// Keep unfinished UTF-16 and every newer native revision while rendering catches up.
    fn text(&mut self, field: PathField, value: Option<&str>, revision: u64) -> Result<()> {
        let slot = Action::Path(field).text_slot().ok_or("Native path revision slot absent")?;
        if revision < self.view.text_revisions[slot] || !self.navigation.flow.can_edit() {
            return Ok(());
        }
        let changed = value.is_some_and(|value| value != self.path(field));
        if changed || revision != self.view.text_revisions[slot] {
            self.invalidate()?;
            self.view.text_revisions[slot] = revision;
            if let Some(value) = value {
                self.navigation.flow.set_path(field, value.to_owned())?;
            }
        }
        Ok(())
    }
    /// Apply one enabled action, invalidating before any changed state can race a completed paint.
    fn action(&mut self, action: Action, input: Input) -> Result<Option<Request>> {
        let now = self.view.now;
        self.invalidate()?;
        let flow = &mut self.navigation.flow;
        Ok(match (action, input) {
            (Action::Primary, Input::Invoke) if flow.can_primary() => flow.primary(now)?,
            (Action::Back, Input::Invoke) if flow.can_back() => {
                flow.back()?;
                None
            }
            (Action::Locale, Input::Select(Some(index))) if flow.can_edit() => {
                let locale = sugarsubstitute_contracts::localization::LOCALES
                    .get(index)
                    .ok_or("Native locale index invalid")?;
                if flow.choices().locale == locale.id {
                    None
                } else {
                    Some(flow.set_locale(locale.id, now)?)
                }
            }
            (Action::Runtime(mode), Input::Invoke) if flow.can_edit() => {
                flow.set_mode(mode)?;
                None
            }
            (Action::Option(field), Input::Select(Some(index))) if flow.can_edit() => {
                if flow.selected(field) == Some(index) {
                    None
                } else {
                    flow.select_option(field, index, now)?
                }
            }
            (Action::Mirrors, Input::Invoke) if flow.can_edit() => {
                Some(flow.set_mirrors(!flow.mirrors(), now)?)
            }
            (Action::Details, Input::Invoke) => {
                flow.set_details(!flow.details_open());
                None
            }
            (Action::Browse(field), Input::Invoke) if flow.can_edit() => {
                self.browse(field)?;
                None
            }
            _ => None,
        })
    }
    /// Capture document and page identity before opening the native modal picker.
    fn browse(&mut self, field: PathField) -> Result<()> {
        if self.picker.is_some() {
            return Ok(());
        }
        let request = self.next_picker;
        self.next_picker = request.checked_add(1).ok_or("Native picker identity exhausted")?;
        let slot = Action::Path(field).text_slot().ok_or("Native path revision slot absent")?;
        self.picker = Some(PendingPicker {
            request: Picker {
                control: Action::Path(field).id(),
                request,
                kind: if field == PathField::Python {
                    PickerKind::File
                } else {
                    PickerKind::Folder
                },
            },
            field,
            page: self.navigation.flow.page_revision(),
            text_revision: self.view.text_revisions[slot],
        });
        self.picker_queued = true;
        Ok(())
    }
    /// Cancellation changes nothing; stale replies cannot overwrite edits or another page.
    fn picked(
        &mut self,
        request: Picker,
        result: std::result::Result<Option<String>, String>,
    ) -> Result<()> {
        let Some(pending) = self.picker.as_ref() else {
            return Ok(());
        };
        if pending.request != request {
            return Ok(());
        }
        let pending = self.picker.take().ok_or("Native picker identity absent")?;
        self.picker_queued = false;
        let slot =
            Action::Path(pending.field).text_slot().ok_or("Native path revision slot absent")?;
        if !self.navigation.flow.can_edit()
            || pending.page != self.navigation.flow.page_revision()
            || pending.text_revision != self.view.text_revisions[slot]
        {
            return Ok(());
        }
        match result {
            Ok(Some(value)) => {
                self.invalidate()?;
                self.navigation.flow.set_path(pending.field, value)?;
            }
            Ok(None) => {}
            Err(message) => {
                self.invalidate()?;
                self.navigation.flow.input_failed(message)?;
            }
        }
        Ok(())
    }
    /// Borrow the original application-owned value without translating or normalizing user text.
    fn path(&self, field: PathField) -> &str {
        let c = self.navigation.flow.choices();
        match field {
            PathField::Installation => &c.install_root,
            PathField::Comfy => &c.comfy_root,
            PathField::Python => &c.python,
            PathField::Models => &c.models,
            PathField::Outputs => &c.outputs,
        }
    }
}
