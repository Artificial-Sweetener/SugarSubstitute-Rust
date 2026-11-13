// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Turn the owning loop's events into the form lifetime's input.
use super::*;

impl FormWindow {
    pub(super) fn interaction(&self) -> Event {
        Event::Interaction {
            hovered: self.hovered,
            pressed: self.pressed,
            focused: self.controls.as_ref().and_then(Controls::focused),
        }
    }

    pub(super) fn dispatch(
        &mut self,
        input: FormInput<'_>,
        lifetime: &mut impl Lifetime,
    ) -> Result {
        match input {
            FormInput::Action(FormAction::Close) => self.closing = true,
            FormInput::Action(FormAction::Activate(id)) => {
                let observed = self.controls.as_ref().map(|owner| owner.observe(id)).transpose()?;
                if let Some(input) = observed.flatten() {
                    lifetime.input(Event::Control(ControlId::new(id)?, input))?;
                }
            }
            FormInput::Action(FormAction::FocusNext(backward)) => {
                if let Some(owner) = &self.controls {
                    owner.focus_next(backward)?;
                }
            }
            FormInput::Action(FormAction::ScrollPage(key)) => self.scroll.key(key)?,
            FormInput::Action(FormAction::FocusChanged) => lifetime.input(self.interaction())?,
            FormInput::Accent(accent) => lifetime.input(Event::SystemAccent(accent))?,
            FormInput::Redraw if self.identity.is_some() => {
                self.presenter.as_ref().ok_or("Native form presenter absent")?.expose_prepared()?;
                self.scroll.paint()?;
            }
            FormInput::Redraw => {}
            FormInput::Window(event) => self.window_event(event, lifetime)?,
        }
        Ok(())
    }

    pub(super) fn window_event(
        &mut self,
        event: &WindowEvent<'_>,
        lifetime: &mut impl Lifetime,
    ) -> Result {
        match event {
            WindowEvent::CloseRequested => self.closing = lifetime.close_request()?,
            WindowEvent::ThemeChanged(theme) => {
                let presenter = self.presenter.as_mut().ok_or("Native form presenter absent")?;
                presenter.set_theme(*theme == Theme::Dark)?;
                lifetime.input(Event::SystemTheme(paint_theme(*theme)))?;
            }
            WindowEvent::CursorMoved { position, .. } => {
                let scale = self.window.scale_factor();
                self.point = (position.x / scale, position.y / scale);
                let next = if self.scroll.captured() {
                    None
                } else {
                    self.projection
                        .as_ref()
                        .and_then(|value| value.hit(self.point.0, self.point.1, scale))
                };
                if next != self.hovered {
                    self.hovered = next;
                    lifetime.input(self.interaction())?;
                }
            }
            WindowEvent::CursorLeft { .. } => {
                self.hovered = None;
                lifetime.input(self.interaction())?;
            }
            WindowEvent::MouseInput { state, button: MouseButton::Left, .. } => {
                if *state == ElementState::Pressed {
                    self.pressed = self.hovered;
                    if let Some(id) = self.pressed
                        && let Some(owner) = &self.controls
                    {
                        owner.focus(id.get())?;
                    }
                    if self.pressed.is_none()
                        && lifetime
                            .page()
                            .is_some_and(|page| page.draggable(self.point.0, self.point.1))
                    {
                        crate::windows_drag::begin(self.window.as_ref())?;
                    }
                } else {
                    let invoke = self.pressed.take().filter(|id| Some(*id) == self.hovered);
                    if let Some(id) = invoke {
                        lifetime.input(Event::Control(id, Input::Invoke))?;
                    }
                }
                lifetime.input(self.interaction())?;
            }
            WindowEvent::KeyboardInput { event, .. }
                if event.state == ElementState::Pressed && !event.repeat =>
            {
                match event.logical_key {
                    tao::keyboard::Key::Escape => self.closing = lifetime.close_request()?,
                    tao::keyboard::Key::Tab => {
                        if let Some(owner) = &self.controls {
                            owner.focus_next(crate::windows_controls::shift_pressed())?;
                        }
                    }
                    _ => {}
                }
            }
            WindowEvent::Focused(false) => {
                self.hovered = None;
                self.pressed = None;
                lifetime.input(Event::Interaction {
                    hovered: None,
                    pressed: None,
                    focused: None,
                })?;
            }
            _ => {}
        }
        Ok(())
    }
}
