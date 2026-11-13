// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise real form peers through the same native owner used by startup and caption controls.
use super::*;
use sugarsubstitute_ui::form::Role;
use tao::{
    event::Event,
    event_loop::{ControlFlow, EventLoop, EventLoopBuilder},
    platform::{run_return::EventLoopExtRunReturn, windows::EventLoopBuilderExtWindows},
    window::WindowBuilder,
};
use windows::Win32::{
    Foundation::{LPARAM, WPARAM},
    UI::{
        Controls::{EM_GETSEL, EM_REPLACESEL, EM_SETSEL},
        WindowsAndMessaging::*,
    },
};
type TestResult = Result<(), Box<dyn std::error::Error>>;

/// Stable native inventory with two independently editable fields and OS-valued form controls.
fn plan() -> Vec<Specification> {
    let roles = [
        Role::Button,
        Role::TextField("E:\\初期".into()),
        Role::TextField("E:\\Other".into()),
        Role::Select { labels: vec!["First".into(), "第二".into()], selected: Some(1) },
        Role::Radio(true),
        Role::Check(false),
    ];
    roles
        .into_iter()
        .enumerate()
        .map(|(index, role)| Specification {
            control: AccessibleControl {
                id: index as u16 + 1,
                label: "Native field",
                bounds: crate::ControlBounds {
                    x: 10.0,
                    y: 10.0 + index as f64 * 45.0,
                    width: 280.0,
                    height: 33.0,
                },
                close: index == 0,
                visible: true,
                enabled: true,
            },
            role,
            text_revision: 0,
        })
        .collect()
}

#[test]
fn native_form_inventory_rejects_changed_role_before_any_peer_effect() {
    let original = plan();
    assert!(specification::validate(&[], &original).is_ok());
    let mut changed = original.clone();
    changed[1].role = Role::Button;
    assert!(specification::validate(&original, &changed).is_err());
    changed = original.clone();
    changed[1].role = Role::TextField("line\nbreak".into());
    assert!(specification::validate(&original, &changed).is_err());
    changed = original.clone();
    changed[3].role = Role::Select { labels: vec!["Only".into()], selected: Some(1) };
    assert!(specification::validate(&original, &changed).is_err());
    changed = original.clone();
    changed[2].control.id = changed[1].control.id;
    assert!(specification::validate(&[], &changed).is_err());
    changed = original.clone();
    changed[1].control.bounds.width = 20.0;
    assert!(specification::validate(&original, &changed).is_err());
}

#[test]
fn original_form_peers_keep_native_edits_focus_values_and_checked_retirement() -> TestResult {
    crate::windows_test_desktop::run(
        concat!(
            module_path!(),
            "::original_form_peers_keep_native_edits_focus_values_and_checked_retirement"
        ),
        qualify,
    )
}

fn qualify() -> TestResult {
    let mut apartment = crate::windows_ui_apartment::UiApartment::enter()?;
    let mut builder = EventLoopBuilder::<NativeAction>::with_user_event();
    let mut events = builder.with_any_thread(true).build();
    let window = Rc::new(
        WindowBuilder::new()
            .with_visible(false)
            .with_inner_size(tao::dpi::LogicalSize::new(400, 340))
            .build(&events)?,
    );
    let mut plan = plan();
    let mut controls = Controls::new(
        window.clone(),
        events.create_proxy(),
        &plan,
        PaintMode::PreparedPixels,
        None,
    )?;
    let result = (|| -> TestResult {
        controls.initialize()?;
        let frame = frame(&window)?;
        controls.present_fields(&frame, true)?;
        let peer = &controls.creation.peers[1].value;
        let engine = peer.engine.as_ref().ok_or("Native field engine absent")?.clone();
        assert_eq!(engine.field_value()?.text.as_deref(), Some("E:\\初期"));
        let original = peer.hwnd;
        let module = controls.text_library.module().ok_or("Text module absent")?;
        let bounds = values::peer_rect(plan[1].control, &plan[1].role, window.scale_factor())?;
        assert_eq!(peer.rect, bounds);
        // SAFETY: Show and activate only the retained window on the private inactive desktop.
        unsafe {
            let parent = HWND(window.hwnd() as *mut c_void);
            let _previous = ShowWindow(parent, SW_SHOWNOACTIVATE);
            let _previous = windows::Win32::UI::Input::KeyboardAndMouse::SetActiveWindow(parent);
            assert_eq!(
                SendMessageW(original, WM_NCHITTEST, Some(WPARAM(0)), Some(LPARAM(0))).0,
                HTCLIENT as isize
            );
        }
        controls.focus(plan[1].control.id)?;
        assert_eq!(controls.focused().map(|id| id.get()), Some(plan[1].control.id));
        controls.focus_next(false)?;
        // SAFETY: Passive focus observation is restricted to this fixture's creating thread.
        assert_eq!(unsafe { GetFocus() }, controls.creation.peers[2].value.hwnd);
        controls.focus_next(true)?;
        // SAFETY: The same original field remains owned on this inactive desktop.
        assert_eq!(unsafe { GetFocus() }, original);
        let _initial_changes = drain_inputs(&mut events).0;
        engine.message(EM_SETSEL, WPARAM(0), LPARAM(-1))?;
        engine.message(EM_REPLACESEL, WPARAM(1), LPARAM(w!("E:\\最新😀").as_ptr() as isize))?;
        assert_eq!(
            drain_inputs(&mut events).0,
            [plan[1].control.id],
            "Document edits must reach the application even when made through the text engine or accessibility provider"
        );
        engine.message(EM_SETSEL, WPARAM(2), LPARAM(4))?;
        assert!(drain_inputs(&mut events).0.is_empty(), "Selection is not a document edit");
        let selection = engine.message(EM_GETSEL, WPARAM(0), LPARAM(0))?;
        let latest = engine.field_value()?;
        plan[1].control.label = "インストール先";
        controls.update(&plan)?;
        assert_eq!(engine.field_value()?.text, latest.text);
        assert_eq!(engine.message(EM_GETSEL, WPARAM(0), LPARAM(0))?, selection);
        plan[1].role = Role::TextField(latest.text.ok_or("Incomplete native text")?);
        plan[1].text_revision = latest.revision;
        controls.update(&plan)?;
        assert_eq!(controls.creation.peers[1].value.hwnd, original);
        assert_eq!(engine.message(EM_GETSEL, WPARAM(0), LPARAM(0))?, selection);
        assert!(
            drain_inputs(&mut events).0.is_empty(),
            "Acknowledged snapshots must not create edit feedback"
        );
        let value_before_hide = engine.field_value()?;
        plan[1].control.visible = false;
        controls.update(&plan)?;
        controls.present_fields(&frame, true)?;
        assert_eq!(engine.field_value()?.text, value_before_hide.text);
        assert_eq!(engine.field_value()?.revision, value_before_hide.revision);
        assert_eq!(engine.message(EM_GETSEL, WPARAM(0), LPARAM(0))?, selection);
        plan[1].control.visible = true;
        controls.update(&plan)?;
        controls.present_fields(&frame, true)?;
        assert_eq!(controls.creation.peers[1].value.hwnd, original);
        assert_eq!(engine.field_value()?.text, value_before_hide.text);
        assert_eq!(engine.message(EM_GETSEL, WPARAM(0), LPARAM(0))?, selection);
        controls.focus(plan[1].control.id)?;
        // SAFETY: Send a character only to this original field on the private inactive desktop.
        unsafe {
            SendMessageW(original, WM_CHAR, Some(WPARAM(usize::from(b'Z'))), Some(LPARAM(1)))
        };
        assert_eq!(
            drain_inputs(&mut events).0,
            [plan[1].control.id],
            "Window input must notify exactly once"
        );
        plan[1].text_revision = engine.field_value()?.revision;
        plan[1].role = Role::TextField("E:\\Selected folder".into());
        controls.update(&plan)?;
        assert_eq!(engine.field_value()?.text.as_deref(), Some("E:\\Selected folder"));
        plan[3].control.label = "Runtime version";
        plan[3].role = Role::Select {
            labels: vec!["New".into(), "Other".into(), "Last".into()],
            selected: Some(2),
        };
        plan[4].role = Role::Radio(false);
        plan[5].role = Role::Check(true);
        controls.update(&plan)?;
        // SAFETY: Read scalar values only from this retained native inventory.
        unsafe {
            assert_eq!(
                SendMessageW(
                    controls.creation.peers[3].value.hwnd,
                    CB_GETCOUNT,
                    Some(WPARAM(0)),
                    Some(LPARAM(0))
                )
                .0,
                3
            );
            assert_eq!(
                SendMessageW(
                    controls.creation.peers[3].value.hwnd,
                    CB_GETCURSEL,
                    Some(WPARAM(0)),
                    Some(LPARAM(0))
                )
                .0,
                2
            );
            assert_eq!(
                SendMessageW(
                    controls.creation.peers[4].value.hwnd,
                    BM_GETCHECK,
                    Some(WPARAM(0)),
                    Some(LPARAM(0))
                )
                .0,
                0
            );
            assert_eq!(
                SendMessageW(
                    controls.creation.peers[5].value.hwnd,
                    BM_GETCHECK,
                    Some(WPARAM(0)),
                    Some(LPARAM(0))
                )
                .0,
                1
            );
        }
        assert_eq!(accessible_name(controls.creation.peers[3].value.hwnd)?, "Runtime version");
        plan[2].control.visible = false;
        controls.update(&plan)?;
        controls.focus(plan[1].control.id)?;
        assert_eq!(controls.focused().map(|id| id.get()), Some(plan[1].control.id));
        controls.focus_next(false)?;
        // SAFETY: Hidden fields must be omitted by the shared actual-peer focus traversal.
        assert_eq!(unsafe { GetFocus() }, controls.creation.peers[3].value.hwnd);
        assert_eq!(controls.text_library.module(), Some(module));
        let _previous_inputs = drain_inputs(&mut events);
        // SAFETY: Send keys only to the fixture's original prepared, editable and selector peers.
        unsafe {
            SendMessageW(
                controls.creation.peers[0].value.hwnd,
                WM_KEYDOWN,
                Some(WPARAM(0x22)),
                Some(LPARAM(1)),
            );
            SendMessageW(original, WM_KEYDOWN, Some(WPARAM(0x23)), Some(LPARAM(1)));
            SendMessageW(
                controls.creation.peers[3].value.hwnd,
                WM_KEYDOWN,
                Some(WPARAM(0x24)),
                Some(LPARAM(1)),
            );
        }
        assert_eq!(
            drain_inputs(&mut events).1,
            [sugarsubstitute_ui::windows::scroll_port::ScrollKey::PageAfter]
        );

        popup::qualify(&controls.creation.peers[3].value)?;
        for theme in [
            sugarsubstitute_contracts::configuration::Theme::Light,
            sugarsubstitute_contracts::configuration::Theme::Dark,
        ] {
            controls.present_fields(
                &frame,
                theme == sugarsubstitute_contracts::configuration::Theme::Dark,
            )?;
            popup::qualify_palette(&controls.creation.peers[3].value, &frame, theme)?;
        }
        popup::qualify_commit(&controls.creation.peers[3].value, &mut events)?;
        popup::qualify_repaint(&mut controls, &mut plan, &mut events)?;
        Ok(())
    })();
    controls.shutdown()?;
    assert!(controls.retired());
    assert!(controls.text_library.module().is_none());
    drop(controls);
    drop(window);
    drop(events);
    // SAFETY: Every fixture window, peer, engine and callback completed checked teardown.
    unsafe { apartment.release_after_teardown() }?;
    result
}

/// Fund actual field backgrounds and providers independently of raster preparation under test.
fn frame(
    window: &Window,
) -> Result<sugarsubstitute_contracts::presentation::PreparedFrame, Box<dyn std::error::Error>> {
    use sugarsubstitute_contracts::{
        execution::{Generation, MemoryLease, MemoryRelease, SurfaceId},
        presentation::{
            FrameIdentity, FrameShape, FrameValidator, NATIVE_CONSOLE_ACCESSIBILITY_BYTES,
        },
    };
    struct Release;
    impl MemoryRelease for Release {
        fn release(&self, _: u64) {}
    }
    let size = window.inner_size();
    let shape = FrameShape::new(size.width, size.height, window.scale_factor())?;
    let mut pixels = vec![0; shape.byte_len()];
    for pixel in pixels.chunks_exact_mut(4) {
        pixel.copy_from_slice(&[29, 29, 29, 255]);
    }
    let lease = MemoryLease::new(
        4 * shape.byte_len() as u64 + 2 * NATIVE_CONSOLE_ACCESSIBILITY_BYTES,
        Box::new(Release),
    );
    let mut validation = FrameValidator::new(
        shape,
        FrameIdentity {
            surface: SurfaceId::new(1)?,
            state_generation: Generation::new(1)?,
            sequence: Generation::new(1)?,
        },
        pixels,
        lease,
    )?;
    while !validation.validate_next(std::num::NonZeroUsize::MIN.saturating_add(32767))? {}
    Ok(validation.finish()?)
}

/// Pump one real native event-loop turn without waits, input injection or touching the active desktop.
fn drain_inputs(
    events: &mut EventLoop<NativeAction>,
) -> (Vec<u16>, Vec<sugarsubstitute_ui::windows::scroll_port::ScrollKey>) {
    let mut changed = Vec::new();
    let mut scrolled = Vec::new();
    events.run_return(|event, _, control| {
        *control = ControlFlow::Poll;
        match event {
            Event::UserEvent(NativeAction::Activate(id)) => changed.push(id),
            Event::UserEvent(NativeAction::ScrollPage(key)) => scrolled.push(key),
            Event::RedrawEventsCleared => *control = ControlFlow::Exit,
            _ => {}
        }
    });
    (changed, scrolled)
}

/// Query the OS selector's actual accessibility name, independently of displayed selected text.
fn accessible_name(hwnd: HWND) -> Result<String, Box<dyn std::error::Error>> {
    use windows::{
        Win32::{
            System::Variant::VARIANT,
            UI::Accessibility::{AccessibleObjectFromWindow, IAccessible},
        },
        core::Interface,
    };
    let mut raw = std::ptr::null_mut();
    // SAFETY: Query the exact retained fixture child; success transfers one owned COM reference.
    unsafe {
        AccessibleObjectFromWindow(hwnd, OBJID_CLIENT.0 as u32, &IAccessible::IID, &mut raw)
    }?;
    if raw.is_null() {
        return Err("Native accessible selector absent".into());
    }
    // SAFETY: The successful query returned this exact interface with one owned reference.
    let accessible = unsafe { IAccessible::from_raw(raw) };
    // SAFETY: CHILDID_SELF refers only to the retained original native selector.
    Ok(unsafe { accessible.get_accName(&VARIANT::from(0_i32)) }?.to_string())
}

#[path = "windows_controls_popup_tests.rs"]
mod popup;
