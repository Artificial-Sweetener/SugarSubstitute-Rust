// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! The update report opened over a main-like window on that window's own event loop: owned by
//! and centred on it, captured from the frames it presents, closed by the user, settled while
//! the main window stays up, and its shell picker handed back to the loop.
use super::*;
use crate::{
    installer::native::{FormOwner, HostedForm},
    native_action::NativeAction,
};
use ::windows::Win32::{
    Foundation::RECT,
    UI::WindowsAndMessaging::{GW_OWNER, GetWindow, GetWindowRect, IsWindow},
};
use tao::{
    dpi::LogicalSize,
    event::Event as NativeEvent,
    event_loop::{ControlFlow, EventLoopBuilder},
    platform::{
        run_return::EventLoopExtRunReturn,
        windows::{EventLoopBuilderExtWindows, WindowExtWindows},
    },
    window::WindowBuilder,
};

/// What the main-like window saw of the report while it was open.
#[derive(Default)]
struct Seen {
    owned: bool,
    centre_offset: Option<(i32, i32)>,
    picker_returned: bool,
    main_alive_after: bool,
}

fn centre(rect: RECT) -> (i32, i32) {
    ((rect.left + rect.right) / 2, (rect.top + rect.bottom) / 2)
}

fn hosted_body() -> TestResult {
    let title = "SugarSubstitute update report";
    let copy = messages("en");
    let mut export = NoExport;
    let mut inner = UpdateReport::new(Some(&mut export), copy.report);
    let mut script = Script::new(
        title,
        &mut inner,
        vec![
            Step::Capture("update-report-over-main"),
            Step::Click(ReportAction::ToggleDetails),
            Step::Capture("update-report-over-main-details"),
            Step::Click(ReportAction::Close),
        ],
    );
    let shared = script.captures.clone();
    let failure = sugarsubstitute_contracts::update::attempt::UpdateReport {
        version: Some("0.6.0".into()),
        phase: sugarsubstitute_contracts::update::attempt::AttemptPhase::Downloading,
        causes: vec![
            "The download stopped before it finished".into(),
            "The connection was reset".into(),
        ],
    };
    let presentation = ReportPresentation::update_attempt(copy, title, &failure);
    let inner_lifetime = lifetime::ReportLifetime::new(presentation, &mut script)?;
    let observer = Observer { shared: shared.clone(), width: 0, height: 0, pixels: Vec::new() };
    let mut observed = Some(Observed {
        inner: inner_lifetime,
        window: None,
        acknowledged: None,
        closing: false,
        observer,
    });

    let mut builder = EventLoopBuilder::<NativeAction>::with_user_event();
    builder.with_any_thread(true);
    let picker = crate::file_dialog::Picker::new(&mut builder);
    let mut events = builder.build();
    let proxy = events.create_proxy();
    let main = WindowBuilder::new()
        .with_title("Main window qualification")
        .with_inner_size(LogicalSize::new(1280.0, 800.0))
        .with_decorations(false)
        .with_visible(true)
        .build(&events)?;
    let owner = ::windows::Win32::Foundation::HWND(main.hwnd() as *mut _);
    let mut report: Option<HostedForm<Observed<'_>>> = None;
    let mut seen = Seen::default();
    let mut failure_text = None;
    events.run_return(|event, target, control| {
        *control = ControlFlow::WaitUntil(
            std::time::Instant::now() + std::time::Duration::from_millis(20),
        );
        if let Some(lifetime) = observed.take() {
            let window = report_window(title.to_owned());
            match HostedForm::open(
                target,
                proxy.clone(),
                picker.clone(),
                Some(FormOwner(owner)),
                window,
                lifetime,
            ) {
                Ok(opened) => report = Some(opened),
                Err(cause) => {
                    failure_text = Some(cause.to_string());
                    *control = ControlFlow::Exit;
                    return;
                }
            }
        }
        let Some(form) = &mut report else { return };
        match event {
            NativeEvent::WindowEvent { window_id, event, .. } if form.owns_window(window_id) => {
                form.window_event(&event);
            }
            NativeEvent::UserEvent(NativeAction::Form(scope, action)) => form.action(scope, action),
            NativeEvent::RedrawRequested(id) if form.owns_window(id) => form.redraw(),
            NativeEvent::MainEventsCleared => {
                if let Ok(hwnd) =
                    // SAFETY: Read-only lookup on this test's private desktop.
                    unsafe { FindWindowW(PCWSTR::null(), &HSTRING::from(title)) }
                    // SAFETY: Scalar visibility query of the window found above.
                    && unsafe { IsWindowVisible(hwnd) }.as_bool()
                    && seen.centre_offset.is_none()
                {
                    let (mut theirs, mut ours) = (RECT::default(), RECT::default());
                    // SAFETY: Read-only queries of this test's own windows.
                    unsafe {
                        seen.owned = GetWindow(hwnd, GW_OWNER).is_ok_and(|found| found == owner);
                        if GetWindowRect(hwnd, &raw mut theirs).is_ok()
                            && GetWindowRect(owner, &raw mut ours).is_ok()
                        {
                            let (a, b) = (centre(theirs), centre(ours));
                            seen.centre_offset = Some((a.0 - b.0, a.1 - b.1));
                        }
                    }
                }
                let (turn, returned) = form.turn(false);
                seen.picker_returned |= returned.is_some();
                if turn.finished {
                    // SAFETY: Scalar liveness query of the main-like window.
                    seen.main_alive_after = unsafe { IsWindow(Some(owner)) }.as_bool();
                    *control = ControlFlow::Exit;
                }
            }
            _ => {}
        }
    });
    drop(report);
    drop(observed);
    picker.retire()?;
    drop(main);
    assert_eq!(failure_text, None);
    assert_eq!(script.failure, None);
    assert_eq!(script.next, 4, "every scripted step ran");
    assert!(seen.owned, "the report is owned by the main window");
    let (dx, dy) = seen.centre_offset.ok_or("the report was never shown")?;
    assert!(dx.abs() <= 2 && dy.abs() <= 2, "report centre is off by {dx},{dy}");
    assert!(seen.picker_returned, "the loop's picker came back");
    assert!(seen.main_alive_after, "the main window outlives the report");
    let captures = std::mem::take(&mut shared.borrow_mut().taken);
    assert_eq!(captures.len(), 2);
    assert_painted(&captures);
    keep(&captures)
}

#[test]
fn update_report_opens_over_the_main_window_on_its_own_loop_and_settles() -> TestResult {
    crate::windows_test_desktop::run(
        concat!(
            module_path!(),
            "::update_report_opens_over_the_main_window_on_its_own_loop_and_settles"
        ),
        hosted_body,
    )
}
