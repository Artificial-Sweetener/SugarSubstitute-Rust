// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Pump the same window while webview construction and the installer's owned jobs progress.
use super::*;
use crate::{
    completion_wake::{ProxyWake, WakeLifetime},
    native_action::NativeAction,
    windows::Presenter,
    windows_controls::console::Console,
};
use std::{
    rc::Rc,
    time::{Duration, Instant},
};
use tao::{
    dpi::LogicalSize,
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoopBuilder},
    platform::run_return::EventLoopExtRunReturn,
    window::{Theme, WindowBuilder},
};
use wry::{
    ConstructionProgress, WebContext, WebView, WebViewBuilder, WebViewBuilderExtWindows,
    WebViewConstruction, WebViewExtWindows, WebViewRetirement,
};
mod bridge;
mod dialog;

pub(super) fn run(
    options: InstallerWindow,
    lifetime: &mut impl InstallerLifetime,
) -> Result<(), Box<dyn Error>> {
    let mut apartment = crate::windows_ui_apartment::UiApartment::enter()?;
    let mut events = EventLoopBuilder::<NativeAction>::with_user_event().build();
    let proxy = events.create_proxy();
    let wake_lifetime = WakeLifetime::new();
    let window = Rc::new(
        WindowBuilder::new()
            .with_title(options.title)
            .with_inner_size(LogicalSize::new(1180, 760))
            .with_min_inner_size(LogicalSize::new(720, 560))
            .with_max_inner_size(tao::dpi::PhysicalSize::new(4096, 4096))
            .with_decorations(false)
            .with_transparent(true)
            .with_theme(Some(Theme::Dark))
            .with_visible(false)
            .build(&events)?,
    );
    let presenter = Presenter::new_main(window.clone(), true)?;
    let mut context = WebContext::new(Some(options.profile));
    let build_proxy = proxy.clone();
    let builder = WebViewBuilder::new_with_web_context(&mut context)
        .with_url("about:blank")
        .with_visible(false)
        .with_focused(false)
        .with_additional_browser_args("")
        .without_download_handlers()
        .with_bounds(wry::Rect {
            position: tao::dpi::PhysicalPosition::new(0, 0).into(),
            size: window.inner_size().into(),
        })
        .with_transparent(true)
        .with_scroll_bar_style(wry::ScrollBarStyle::FluentOverlay);
    let mut constructor: Option<WebViewConstruction> = None;
    let mut partial: Option<WebViewRetirement> = None;
    let mut view: Option<WebView> = None;
    let mut bridge: Option<bridge::Bridge> = None;
    let mut console: Option<Console> = None;
    let mut console_above_browser = false;
    let mut failure: Option<Box<dyn Error>> = None;
    match builder.begin_owned_as_child(window.as_ref(), move || {
        if let Err(cause) = build_proxy.send_event(NativeAction::Main) {
            tracing::error!(operation="installer_construction", cause=%cause, "Installer wake failed");
        }
    }) {
        Ok(value) => constructor = Some(value),
        Err(error) => {
            let (cause, ownership) = error.into_parts();
            partial = Some(ownership);
            failure = Some(Box::new(cause));
        }
    }
    if let Err(cause) = lifetime.start(Arc::new(ProxyWake::new(
        proxy.clone(),
        wake_lifetime.clone(),
        NativeAction::Completions,
    ))) {
        failure = Some(cause);
    }
    let mut closing = false;
    let mut closed = false;
    let mut revealed = false;
    let mut document_ready = false;
    let mut environment_observed = false;
    let mut last_progress = None;
    let mut outbound = std::collections::VecDeque::new();
    events.run_return(|event, _, control| {
        *control = ControlFlow::WaitUntil(Instant::now() + Duration::from_millis(20));
        let result: Result<(), Box<dyn Error>> = (|| -> Result<(), Box<dyn Error>> {
            match event {
                Event::WindowEvent { event: WindowEvent::CloseRequested, .. } => closing = true,
                Event::UserEvent(NativeAction::Close) => closing = true,
                Event::UserEvent(NativeAction::FocusNext(backward)) => {
                    if let Some(view) = &view { view.focus()?; }
                    if let Some(bridge) = &bridge {
                        bridge.post(&serde_json::json!({"event":"console_focus_exit","backward":backward}).to_string())?;
                    }
                }
                Event::WindowEvent { event: WindowEvent::Resized(size), .. } => {
                    console_above_browser = false;
                    if let Some(view) = &view {
                        view.set_bounds(wry::Rect {
                            position: tao::dpi::PhysicalPosition::new(0, 0).into(),
                            size: size.into(),
                        })?;
                    }
                }
                Event::MainEventsCleared => {
                    for _ in 0..16 {
                    let Some(body) = bridge.as_ref().map(|bridge| bridge.next()).transpose()?.flatten() else { break; };
                    if closing { continue; }
                    let value: serde_json::Value = serde_json::from_str(&body)?;
                    if value["version"] != 1 || !value["id"].is_string() {
                        return Err("Invalid installer document request".into());
                    }
                    if value["operation"] == "ready" {
                        if let Some(bridge) = &bridge { bridge.document_ready(); }
                        tracing::info!(operation="installer_document_ready", "Installer document connected");
                        document_ready = true;
                    } else if value["operation"] == "console" {
                        let input = &value["input"];
                        let number = |key| input[key].as_f64().ok_or("Console geometry missing");
                        let size = window.inner_size();
                        lifetime.console_layout(InstallerConsoleLayout {
                            width: size.width, height: size.height, scale: window.scale_factor(),
                            bounds: crate::ControlBounds { x: number("x")?, y: number("y")?, width: number("width")?, height: number("height")? },
                            visible: input["visible"].as_bool().ok_or("Console visibility missing")?,
                        })?;
                    } else if value["operation"] == "browse" {
                        let result = dialog::choose(window.as_ref(), value["input"]["field"].as_str().unwrap_or_default());
                        let message = match result {
                            Ok(path) => serde_json::json!({"event":"completed","id":value["id"],"data":path}),
                            Err(cause) => serde_json::json!({"event":"failed","id":value["id"],"data":{"message":cause.to_string()}}),
                        };
                        if outbound.len() >= 32 { return Err("Installer document queue exceeded".into()); }
                        outbound.push_back(message.to_string());
                    } else if value["operation"] == "drag" {
                        window.drag_window()?;
                    } else if value["operation"] == "close" {
                        closing = true;
                    } else {
                        lifetime.request(&body)?;
                    }
                    }
                    if failure.is_some() { closing = true; }
                    if closing && !closed {
                        closed = true;
                        lifetime.close()?;
                        if let Some(bridge) = &bridge {
                            bridge.post("{\"event\":\"closing\"}")?;
                        }
                        if let Some(constructor) = &mut constructor { constructor.cancel(); }
                    }
                    let messages = lifetime.advance()?;
                    if messages.len() > 16 || messages.iter().any(|message| message.len() > 65536) {
                        return Err("Installer output exceeded the native delivery bound".into());
                    }
                    if outbound.len() + messages.len() > 32 { return Err("Installer document queue exceeded".into()); }
                    outbound.extend(messages);
                    if let Some(bridge) = &bridge
                        && document_ready
                    {
                        for message in outbound.drain(..) {
                            bridge.post(&message)?;
                        }
                    }
                    if !closing {
                        if let Some(pending) = &mut constructor {
                            if pending.take_close_requested() { closing = true; }
                            let progress = pending.progress()?;
                            if last_progress != Some(progress) {
                                tracing::info!(operation="installer_construction", ?progress, "Installer construction advanced");
                                last_progress = Some(progress);
                            }
                            if progress == ConstructionProgress::EnvironmentPrepared {
                                if !environment_observed {
                                    if let Some(folder) = pending.user_data_folder()? {
                                        use std::os::windows::ffi::OsStringExt;
                                        lifetime.environment_observed(PathBuf::from(std::ffi::OsString::from_wide(&folder.into_utf16())))?;
                                        environment_observed = true;
                                    }
                                } else if lifetime.environment_ready() {
                                    pending.authorize_environment()?;
                                }
                            }
                            if progress == ConstructionProgress::Finished {
                                let result = pending.take_result().ok_or("Installer controller result missing")?;
                                constructor = None;
                                match result {
                                    Ok(value) => {
                                        view = Some(value);
                                        let current = view.as_ref().ok_or("Installer view absent")?;
                                        bridge = Some(bridge::Bridge::new(current.webview(), proxy.clone()));
                                        bridge.as_mut().ok_or("Installer bridge absent")?.install()?;
                                        current.load_html(&options.html)?;
                                        tracing::info!(operation="installer_document_load", "Installer document submitted");
                                        current.set_visible(true)?;
                                    },
                                    Err(error) => {
                                        let (cause, ownership) = error.into_parts();
                                        partial = Some(ownership);
                                        return Err(Box::new(cause));
                                    }
                                }
                            }
                        }
                        if view.is_some() && document_ready && !revealed {
                            window.set_visible(options.visible);
                            if options.visible { window.set_focus(); }
                            revealed = true;
                        }
                        if let Some(prepared) = lifetime.console() {
                            let first = console.is_none() && prepared.view.visible;
                            if console.is_none() && prepared.view.visible {
                                console = Some(Console::new(proxy.clone(), None, prepared.frame, &prepared.view)?);
                                console.as_mut().ok_or("Installer console absent")?.initialize(window.as_ref(), &prepared.view)?;
                            }
                            if let Some(console) = &mut console {
                                console.present(window.as_ref(), prepared.frame, &prepared.view)?;
                                if prepared.view.visible && !console_above_browser {
                                    console.raise()?;
                                }
                                console_above_browser = prepared.view.visible;
                            }
                            if first && let Some(bridge) = &bridge {
                                bridge.post("{\"event\":\"native_console_ready\"}")?;
                            }
                        }
                    } else if lifetime.settled()? {
                        if let Some(owner) = &mut console {
                            owner.shutdown(window.as_ref())?;
                            if !owner.retired() { return Ok(()); }
                            console = None;
                        }
                        if let Some(owner) = &mut bridge {
                            if !owner.retire()? { return Ok(()); }
                            bridge = None;
                        }
                        if retire(&mut constructor, &mut partial, &mut view)? {
                            *control = ControlFlow::Exit;
                        }
                    }
                }
                _ => {}
            }
            Ok(())
        })();
        if let Err(cause) = result {
            tracing::error!(operation="installer_window", cause=%cause, "Installer entered cleanup");
            if failure.is_none() { failure = Some(cause); }
            closing = true;
        }
    });
    wake_lifetime.disconnect();
    drop(console);
    drop(bridge);
    drop(view);
    drop(partial);
    drop(constructor);
    drop(context);
    drop(presenter);
    drop(window);
    drop(events);
    // SAFETY: The loop exits only after checked webview retirement; all retained
    // webview/context/window/event-loop owners above have now been dropped on this thread.
    unsafe { apartment.release_after_teardown()? };
    lifetime.finish()?;
    match failure {
        Some(cause) => Err(cause),
        None => Ok(()),
    }
}

fn retire(
    constructor: &mut Option<WebViewConstruction>,
    partial: &mut Option<WebViewRetirement>,
    view: &mut Option<WebView>,
) -> Result<bool, Box<dyn Error>> {
    if let Some(value) = constructor {
        if value.checked_retire()? {
            *constructor = None;
        }
        return Ok(false);
    }
    if let Some(value) = partial {
        if value.checked_retire()? {
            *partial = None;
        }
        return Ok(false);
    }
    if let Some(value) = view {
        if value.checked_retire()? {
            *view = None;
        }
        return Ok(false);
    }
    Ok(true)
}
