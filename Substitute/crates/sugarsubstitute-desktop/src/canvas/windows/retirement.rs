// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Observe destruction of the original canvas peer before releasing its input and UI apartment.
use super::*;

/// Pump only the owning event loop until Tao reports the original HWND destroyed.
/// The picker must already be settled and retired; native input callbacks retain their resources
/// until final native destruction. The deadline reports uncertainty, never settlement.
/// # Errors
/// Reports unobserved window or callback destruction without releasing uncertain native custody.
pub(super) fn destroy(
    events: &mut tao::event_loop::EventLoop<NativeEvent>,
    window: Arc<tao::window::Window>,
    native: Option<&NativeInput>,
) -> Result<(), Box<dyn Error>> {
    let original = window.id();
    drop(window);
    let deadline = Instant::now() + Duration::from_secs(15);
    let mut destroyed = false;
    events.run_return(|event, _, control| {
        *control = ControlFlow::WaitUntil(deadline);
        match event {
            Event::WindowEvent { window_id, event: WindowEvent::Destroyed, .. }
                if window_id == original =>
            {
                destroyed = true;
                *control = ControlFlow::Exit;
            }
            Event::MainEventsCleared if Instant::now() >= deadline => *control = ControlFlow::Exit,
            _ => {}
        }
    });
    if !destroyed || native.is_some_and(|native| !native.retired()) {
        return Err(DesktopError::message(
            "canvas_window_retirement",
            "Native canvas destruction was not observed",
        )
        .into());
    }
    Ok(())
}
