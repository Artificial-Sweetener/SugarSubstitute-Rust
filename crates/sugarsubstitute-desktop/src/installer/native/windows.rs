// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Present prepared installer pages using the original native window and shared form peers.
use super::*;
use crate::{
    completion_wake::{ProxyWake, WakeLifetime},
    native_action::NativeAction,
    windows::Presenter,
    windows_controls::{Controls, PaintMode},
    windows_scroll::ScrollViewport,
};
use std::{rc::Rc, time::Instant};
use tao::{
    dpi::LogicalSize,
    event::{ElementState, Event as NativeEvent, MouseButton, WindowEvent},
    event_loop::{ControlFlow, EventLoopBuilder},
    platform::{run_return::EventLoopExtRunReturn, windows::WindowExtWindows},
    window::{Theme, WindowBuilder},
};
#[path = "projection.rs"]
mod projection;
use projection::Projection;

/// Translate the same theme resolved by the native window into shared preparation input.
fn paint_theme(theme: Theme) -> sugarsubstitute_contracts::configuration::Theme {
    if theme == Theme::Dark {
        sugarsubstitute_contracts::configuration::Theme::Dark
    } else {
        sugarsubstitute_contracts::configuration::Theme::Light
    }
}

/// Own a single event loop from hidden construction through checked native and application teardown.
pub(super) fn run(
    options: super::Window,
    lifetime: &mut impl Lifetime,
) -> Result<(), Box<dyn Error>> {
    let mut apartment = crate::windows_ui_apartment::UiApartment::enter()?;
    let mut builder = EventLoopBuilder::<NativeAction>::with_user_event();
    #[cfg(any(test, feature = "boundary-fixtures"))]
    {
        use tao::platform::windows::EventLoopBuilderExtWindows;
        builder.with_any_thread(true);
    }
    let picker = crate::file_dialog::Picker::new(&mut builder);
    let mut events = builder.build();
    let proxy = events.create_proxy();
    let wake_lifetime = WakeLifetime::new();
    let mut builder = WindowBuilder::new()
        .with_title(options.title)
        .with_inner_size(LogicalSize::new(options.size[0], options.size[1]))
        .with_resizable(options.resizable)
        .with_max_inner_size(tao::dpi::PhysicalSize::new(4096, 4096))
        .with_decorations(false)
        .with_transparent(true)
        .with_theme(None)
        .with_visible(false);
    if let Some([width, height]) = options.min_size {
        builder = builder.with_min_inner_size(LogicalSize::new(width, height));
    }
    let window = Rc::new(builder.build(&events)?);
    let (mut presenter, mut failure): (_, Option<Box<dyn Error>>) =
        match Presenter::new_main(window.clone(), window.theme() == Theme::Dark) {
            Ok(owner) => (Some(owner), None),
            Err(cause) => (None, Some(Box::new(cause))),
        };
    let picker_proxy = proxy.clone();
    let bound =
        picker.bind(::windows::Win32::Foundation::HWND(window.hwnd() as *mut _), move || {
            picker_proxy
                .send_event(NativeAction::Completions)
                .map_err(|cause| crate::DesktopError::new("installer_picker_wake", cause))
        });
    if let Err(cause) = bound
        && failure.is_none()
    {
        failure = Some(Box::new(cause));
    }
    let mut picker_request = None;
    let mut applied_size = None;
    match presenter.as_ref().map(Presenter::work_area_height).transpose() {
        Ok(Some(height)) => lifetime.work_area(height),
        Ok(None) => {}
        Err(cause) => {
            if failure.is_none() {
                failure = Some(Box::new(cause));
            }
        }
    }
    let scroll = ScrollViewport::new(window.clone());
    if let Err(cause) = scroll.initialize()
        && failure.is_none()
    {
        failure = Some(Box::new(cause));
    }
    let mut controls: Option<Controls> = None;
    let mut storage = None;
    let mut projection: Option<Projection> = None;
    let mut identity = None;
    let mut shape = None;
    let mut hovered = None;
    let mut pressed = None;
    let mut point = (0.0, 0.0);
    let mut closing = false;
    let mut close_started = false;
    let mut revealed = false;
    let started = Instant::now();
    // Resolve before start so the first prepared page and native peers share this window's mode.
    if let Err(cause) = lifetime.input(Event::SystemTheme(paint_theme(window.theme())))
        && failure.is_none()
    {
        failure = Some(cause);
    }
    let mut accent =
        crate::windows_system_accent::SystemAccent::observe(proxy.clone(), wake_lifetime.clone());
    // The first installer page is prepared with the current accent.
    if let Some(current) = accent.take_change()
        && let Err(cause) = lifetime.input(Event::SystemAccent(current))
        && failure.is_none()
    {
        failure = Some(cause);
    }
    if let Err(cause) = lifetime.start(Arc::new(ProxyWake::new(
        proxy.clone(),
        wake_lifetime.clone(),
        NativeAction::Completions,
    ))) {
        tracing::error!(operation="native_installer_start", cause=%cause, "Installer start failed");
        if failure.is_none() {
            failure = Some(cause);
        }
    }
    events.run_return(|event, _, control| {
        *control = ControlFlow::WaitUntil(Instant::now() + Duration::from_millis(20));
        let result: Result<(), Box<dyn Error>> = (|| {
            match event {
                NativeEvent::WindowEvent { event: WindowEvent::CloseRequested, .. }
                | NativeEvent::UserEvent(NativeAction::Close) => closing = true,
                NativeEvent::UserEvent(NativeAction::Activate(id)) if !closing => {
                    if let Some(input) = controls.as_ref().map(|owner| owner.observe(id)).transpose()?.flatten() {
                        lifetime.input(Event::Control(ControlId::new(id)?, input))?;
                    }
                }
                NativeEvent::UserEvent(NativeAction::FocusNext(backward)) if !closing => {
                    if let Some(owner) = &controls { owner.focus_next(backward)?; }
                }
                NativeEvent::UserEvent(NativeAction::ScrollPage(key)) if !closing => scroll.key(key)?,
                NativeEvent::UserEvent(NativeAction::SystemAccent) if !closing => {
                    if let Some(current) = accent.take_change() { lifetime.input(Event::SystemAccent(current))?; }
                }
                NativeEvent::UserEvent(NativeAction::FocusChanged) if !closing => {
                    lifetime.input(Event::Interaction { hovered, pressed, focused: controls.as_ref().and_then(Controls::focused) })?;
                }
                NativeEvent::WindowEvent { event: WindowEvent::ThemeChanged(theme), .. } if !closing => {
                    presenter.as_mut().ok_or("Native installer presenter absent")?.set_theme(theme == Theme::Dark)?;
                    lifetime.input(Event::SystemTheme(paint_theme(theme)))?;
                }
                NativeEvent::WindowEvent { event: WindowEvent::CursorMoved { position, .. }, .. } if !closing => {
                    let scale = window.scale_factor();
                    point = (position.x / scale, position.y / scale);
                    let next = if scroll.captured() { None } else { projection.as_ref().and_then(|value| value.hit(point.0, point.1, scale)) };
                    if next != hovered {
                        hovered = next;
                        lifetime.input(Event::Interaction { hovered, pressed, focused: controls.as_ref().and_then(Controls::focused) })?;
                    }
                }
                NativeEvent::WindowEvent { event: WindowEvent::CursorLeft { .. }, .. } if !closing => {
                    hovered = None;
                    lifetime.input(Event::Interaction { hovered, pressed, focused: controls.as_ref().and_then(Controls::focused) })?;
                }
                NativeEvent::WindowEvent { event: WindowEvent::MouseInput { state, button: MouseButton::Left, .. }, .. }
                    if !closing => {
                    if state == ElementState::Pressed {
                        pressed = hovered;
                        if let Some(id) = pressed && let Some(owner) = &controls { owner.focus(id.get())?; }
                        if pressed.is_none() && lifetime.page().is_some_and(|page| page.draggable(point.0, point.1)) { crate::windows_drag::begin(window.as_ref())?; }
                    } else {
                        let invoke = pressed.take().filter(|id| Some(*id) == hovered);
                        if let Some(id) = invoke { lifetime.input(Event::Control(id, Input::Invoke))?; }
                    }
                    lifetime.input(Event::Interaction { hovered, pressed, focused: controls.as_ref().and_then(Controls::focused) })?;
                }
                NativeEvent::WindowEvent { event: WindowEvent::KeyboardInput { event, .. }, .. }
                    if !closing && event.state == ElementState::Pressed && !event.repeat => {
                    match event.logical_key {
                        tao::keyboard::Key::Escape => closing = true,
                        tao::keyboard::Key::Tab => if let Some(owner) = &controls { owner.focus_next(crate::windows_controls::shift_pressed())?; },
                        _ => {}
                    }
                }
                NativeEvent::WindowEvent { event: WindowEvent::Focused(false), .. } if !closing => {
                    hovered = None;
                    pressed = None;
                    lifetime.input(Event::Interaction { hovered, pressed, focused: None })?;
                }
                NativeEvent::RedrawRequested(_) if identity.is_some() && !closing => {
                    presenter.as_ref().ok_or("Native installer presenter absent")?.expose_prepared()?;
                    scroll.paint()?;
                },
                NativeEvent::MainEventsCleared => {
                    if let Some(owner) = &controls {owner.check()?;}
                    picker.check()?;
                    closing |= failure.is_some() || lifetime.close_requested();
                    if closing { picker.cancel()?; }
                    if closing && !close_started {
                        lifetime.close()?;
                        scroll.shutdown()?;
                        close_started = true;
                    }
                    if !closing {
                        let size = window.inner_size();
                        if size.width > 0 && size.height > 0 {
                            let next = FrameShape::new(size.width, size.height, window.scale_factor())?;
                            if shape != Some(next) { lifetime.resize(next)?; shape = Some(next); }
                        }
                    }
                    if let Some(result) = picker.take_result() {
                        let request = picker_request.take().ok_or("Native picker returned without a request")?;
                        if !closing {
                            let result = result.and_then(|path| path.map(|path| path.into_os_string().into_string().map_err(|_| {
                                Box::<dyn Error>::from("Selected path cannot be represented in interface text")
                            })).transpose()).map_err(|cause| cause.to_string());
                            lifetime.input(Event::Picked(request, result))?;
                        } else { result?; }
                    }
                    if !closing && !picker.busy() && let Some(request) = lifetime.picker() {
                        picker.request(request.kind)?;
                        picker_request = Some(request);
                    }
                    if !closing && let Some(position) = scroll.take_position()? {
                        lifetime.input(Event::Scroll(position))?;
                    }
                    lifetime.advance(started.elapsed())?;
                    if !closing && let Some(text) = lifetime.clipboard() {
                        let copied = crate::windows_clipboard::copy_text(::windows::Win32::Foundation::HWND(window.hwnd() as *mut _), &text);
                        lifetime.copied(copied.map_err(|cause| cause.to_string()))?;
                    }
                    if !closing && let Some(size) = lifetime.preferred_size() {
                        if Some(size) != applied_size {
                            apply_client_size(&window, size);
                            applied_size = Some(size);
                            if !revealed { presenter.as_ref().ok_or("Native installer presenter absent")?.recenter()?; }
                        }
                        // Acknowledge every turn with the size the window really has, so a
                        // lifetime waiting on its request prepares for this window, not the request.
                        let current = window.inner_size();
                        if current.width > 0 && current.height > 0 {
                            let next = FrameShape::new(current.width, current.height, window.scale_factor())?;
                            shape = Some(next);
                            lifetime.resize(next)?;
                        }
                    }
                    if !closing {
                        if let Some(page) = lifetime.page()
                            && Some(page.frame.shape()) == shape
                            && Some(page.frame.identity()) != identity {
                            let next = Projection::new(lifetime.inventory(), &page, options.close)?;
                            scroll.present(page.frame, page.viewport, page.content_height, page.scroll)?;
                            if controls.is_none() {
                                let fields = lifetime.inventory().iter().filter(|item| matches!(item.role, sugarsubstitute_ui::form::Role::TextField(_))).count();
                                storage = Some(page.frame.claim_storage(2 * 1024 * 1024 * (fields as u64 + 1))?);
                                let mut hidden = next.specifications.clone();
                                for spec in &mut hidden { spec.control.visible = false; }
                                controls = Some(Controls::new(window.clone(), proxy.clone(), &hidden, PaintMode::PreparedPixels, None)?);
                                controls.as_mut().ok_or("Native form owner absent")?.initialize()?;
                            }
                            let owner = controls.as_mut().ok_or("Native form owner absent")?;
                            owner.update(&next.specifications)?;
                            owner.clip(&next.clips)?;
                            owner.present_fields(page.frame, window.theme() == Theme::Dark)?;
                            page.frame.fund_storage(storage.as_mut().ok_or("Native control storage absent")?)?;
                            let painter = presenter.as_mut().ok_or("Native installer presenter absent")?;
                            painter.present(page.frame, None)?;
                            identity = Some(page.frame.identity());
                            projection = Some(next);
                            if !revealed {
                                window.set_visible(options.visible);
                                if options.visible { window.set_focus(); }
                                revealed = true;
                            }
                            painter.expose_prepared()?;
                            let observations = next_observations(owner, projection.as_ref().ok_or("Native page projection absent")?)?;
                            for (id, input) in observations { lifetime.input(Event::Control(id, input))?; }
                            lifetime.presented()?;
                        }
                        if let Some(prepared) = lifetime.console() && let Some(owner) = &mut controls {
                            owner.present_console(prepared.frame, Some(prepared.view), prepared.focus_return.map(|id| id.get()))?;
                        }
                        scroll.paint()?;
                        #[cfg(any(test, feature = "boundary-fixtures"))]
                        if let Some(frame) = lifetime.page().filter(|page| Some(page.frame.identity()) == identity).map(|page| page.frame.clone())
                            && let Some(observer) = lifetime.pixel_capture() {
                            observer.surface(&frame);
                            controls.as_ref().ok_or("Native capture controls absent")?.capture(&projection.as_ref().ok_or("Native capture projection absent")?.clips, observer)?;
                            scroll.capture(observer)?;
                            observer.finished();
                        }
                    } else if lifetime.settled()? && !picker.busy() {
                        if let Some(owner) = &mut controls {
                            owner.shutdown()?;
                            if !owner.retired() { return Ok(()); }
                        }
                        controls = None;
                        projection = None;
                        storage = None;
                        if let Some(owner) = &mut presenter { owner.shutdown()?; }
                        *control = ControlFlow::Exit;
                    }
                }
                _ => {}
            }
            Ok(())
        })();
        if let Err(cause) = result {
            tracing::error!(operation="native_installer_window", cause=%cause, "Installer entered cleanup");
            if failure.is_none() { failure = Some(cause); }
            closing = true;
        }
    });
    accent.retire();
    picker.retire()?;
    drop(picker);
    wake_lifetime.disconnect();
    drop(scroll);
    drop(controls);
    drop(projection);
    drop(storage);
    drop(presenter);
    let original = window.id();
    drop(window);
    // Tao posts its destruction request from Drop. Keep pumping the same event loop until the
    // original OS window reports destruction, rather than mistaking an Rc drop for retirement.
    let deadline = Instant::now() + Duration::from_secs(15);
    let mut destroyed = false;
    events.run_return(|event, _, control| {
        *control = ControlFlow::WaitUntil(deadline);
        match event {
            NativeEvent::WindowEvent { window_id, event: WindowEvent::Destroyed, .. }
                if window_id == original =>
            {
                destroyed = true;
                *control = ControlFlow::Exit;
            }
            NativeEvent::MainEventsCleared if Instant::now() >= deadline => {
                *control = ControlFlow::Exit
            }
            _ => {}
        }
    });
    if !destroyed {
        return Err("Native installer window destruction was not observed".into());
    }
    drop(events);
    // SAFETY: Checked peers, render buffers and their event-loop window retired on this thread.
    unsafe { apartment.release_after_teardown()? };
    let finished = lifetime.finish();
    match (failure, finished) {
        (Some(cause), Err(retirement)) => {
            Err(format!("{cause}; native installer finalization: {retirement}").into())
        }
        (Some(cause), _) => Err(cause),
        (None, result) => result,
    }
}

/// Give the window `size` physical client pixels. Tao enlarges a request for an undecorated
/// window with a shadow by its hidden frame, and not on every call, so each observed difference
/// is fed back into the next request. A window that still differs keeps its real size; the
/// lifetime is told that size and prepares for it.
fn apply_client_size(window: &tao::window::Window, size: [u32; 2]) {
    let mut request = size;
    for _ in 0..3 {
        window.set_inner_size(tao::dpi::PhysicalSize::new(request[0], request[1]));
        let actual = window.inner_size();
        if [actual.width, actual.height] == size {
            return;
        }
        let adjust = |request: u32, wanted: u32, got: u32| {
            (i64::from(request) + i64::from(wanted) - i64::from(got)).clamp(1, 4096) as u32
        };
        request =
            [adjust(request[0], size[0], actual.width), adjust(request[1], size[1], actual.height)];
    }
}

/// Initial document population and picker replacements also advance native edit revisions.
/// Return only mismatches so acknowledgement does not create an endless repaint loop.
fn next_observations(
    owner: &Controls,
    page: &Projection,
) -> Result<Vec<(ControlId, Input)>, Box<dyn Error>> {
    let mut result = Vec::new();
    for spec in &page.specifications {
        if matches!(spec.role, sugarsubstitute_ui::form::Role::TextField(_))
            && let Some(input @ Input::Text { revision, .. }) = owner.observe(spec.control.id)?
            && revision != spec.text_revision
        {
            result.push((ControlId::new(spec.control.id)?, input));
        }
    }
    Ok(result)
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
