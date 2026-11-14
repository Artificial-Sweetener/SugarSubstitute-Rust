// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Show every update caption state on real Main caption peers on a private desktop and keep
//! what the caption paints: each visible peer's owner-drawn frame, prepared into this test's
//! own memory bitmap through the caption's paint path, never read back from the screen. Set
//! `SUGARSUBSTITUTE_REPORT_PROOF_DIR` to keep the captures as bitmaps.
use super::*;
use crate::{
    MainBackendState, MainCaptionLabels, MainRecoveryLabels, WorkflowActivity,
    main_backend::Recovery,
    main_caption::{PEERS, caption_controls},
    main_updates::{CaptionUpdates, Notice, UpdateFeed},
};
use sugarsubstitute_contracts::{
    execution::Generation,
    lifecycle::{CoordinatorEpoch, CoordinatorKey, InstanceId, NamespaceId, RunGeneration, RunKey},
    localization::messages,
    startup::LaunchId,
};
use tao::{dpi::LogicalSize, event_loop::EventLoopBuilder, window::WindowBuilder};
use windows::Win32::{
    Foundation::{LPARAM, WPARAM},
    Graphics::Gdi::{
        BI_RGB, BITMAPINFO, BITMAPINFOHEADER, CreateCompatibleDC, CreateDIBSection, DIB_RGB_COLORS,
        DeleteDC, DeleteObject, HBITMAP, HDC, SelectObject,
    },
    UI::{
        Controls::{DRAWITEMSTRUCT, ODA_DRAWENTIRE, ODS_DEFAULT, ODT_BUTTON},
        WindowsAndMessaging::{
            GetDlgItem, GetWindowRect, IsWindowVisible, SendMessageW, WM_DRAWITEM,
        },
    },
};

pub(super) type TestResult = Result<(), Box<dyn std::error::Error>>;

/// Logical client width the caption is laid out for, and the logical caption strip height.
pub(super) const WIDTH: i32 = 1100;
const HEIGHT: i32 = 34;
pub(super) const LABELS: MainCaptionLabels = MainCaptionLabels {
    minimize: "Minimize",
    maximize: "Maximize",
    restore: "Restore",
    close: "Close",
};

pub(super) fn identity() -> Result<crate::MainIdentity, Box<dyn std::error::Error>> {
    Ok(crate::MainIdentity {
        run: RunKey {
            owner: CoordinatorKey {
                namespace: NamespaceId::new([1; 32]),
                epoch: CoordinatorEpoch::new([2; 16])?,
            },
            instance: InstanceId::new(1)?,
            generation: RunGeneration::new(1)?,
        },
        launch: LaunchId::new(1)?,
        navigation: Generation::new(1)?,
    })
}

/// One caption state, set up from fake application facts.
struct State {
    name: &'static str,
    caption: CaptionUpdates,
    shown: &'static [u16],
}

fn states() -> Vec<State> {
    let feed = |glyph, saving, workflow| UpdateFeed { glyph, saving, workflow };
    let ready = feed(crate::main_updates::UpdateGlyph::Ready, false, WorkflowActivity::Idle);
    let mut states = Vec::new();
    states.push(State {
        name: "caption-up-to-date",
        caption: CaptionUpdates::default(),
        shown: &[],
    });
    let mut caption = CaptionUpdates::default();
    caption.observe(ready);
    states.push(State { name: "caption-update-ready", caption, shown: &[5] });
    let mut caption = CaptionUpdates::default();
    caption.observe(feed(crate::main_updates::UpdateGlyph::Warning, false, WorkflowActivity::Idle));
    states.push(State { name: "caption-update-warning", caption, shown: &[5] });
    let mut caption = CaptionUpdates::default();
    caption.observe(ready);
    caption.activate(5);
    states.push(State { name: "caption-restart-offer", caption, shown: &[5, 6] });
    let mut caption = CaptionUpdates::default();
    caption.observe(feed(
        crate::main_updates::UpdateGlyph::Ready,
        false,
        WorkflowActivity::Running,
    ));
    caption.activate(5);
    states.push(State { name: "caption-restart-after-workflow", caption, shown: &[5, 6] });
    let mut caption = CaptionUpdates::default();
    caption.observe(feed(crate::main_updates::UpdateGlyph::Ready, true, WorkflowActivity::Idle));
    states.push(State { name: "caption-saving", caption, shown: &[7] });
    let mut caption = CaptionUpdates::default();
    caption.notify(Notice::NotSaved { workspace: true, inputs: false });
    states.push(State { name: "caption-not-saved-notice", caption, shown: &[7, 9] });
    let mut caption = CaptionUpdates::default();
    let notes = url::Url::parse("https://github.com/example/releases/tag/v2.4.1").ok();
    caption.notify(Notice::update(crate::UpdateNotice::Updated {
        version: "2.4.1".into(),
        changelog: notes,
    }));
    states.push(State { name: "caption-updated-to-version", caption, shown: &[7, 8, 9] });
    let mut caption = CaptionUpdates::default();
    caption.notify(Notice::update(crate::UpdateNotice::PreviousVersionRunning));
    states.push(State { name: "caption-previous-version-running", caption, shown: &[7, 9] });
    states
}

/// An owned top-down 32-bit memory bitmap one caption peer paints into, as Windows gives an
/// owner-drawn button its own client rectangle.
struct Surface {
    dc: HDC,
    bitmap: HBITMAP,
    previous: windows::Win32::Graphics::Gdi::HGDIOBJ,
    bits: *mut u8,
    width: i32,
    height: i32,
}

impl Surface {
    fn new(width: i32, height: i32) -> Result<Self, Box<dyn std::error::Error>> {
        let info = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: width,
                biHeight: -height,
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..Default::default()
            },
            ..Default::default()
        };
        let mut bits = std::ptr::null_mut();
        // SAFETY: An owned memory DC and DIB section display nothing; Drop releases both.
        unsafe {
            let dc = CreateCompatibleDC(None);
            let bitmap = CreateDIBSection(Some(dc), &info, DIB_RGB_COLORS, &raw mut bits, None, 0)?;
            let previous = SelectObject(dc, bitmap.into());
            Ok(Self { dc, bitmap, previous, bits: bits.cast(), width, height })
        }
    }

    fn pixels(&self) -> &[u8] {
        // SAFETY: The DIB section is exactly width*height 32-bit pixels and lives with self.
        unsafe { std::slice::from_raw_parts(self.bits, (self.width * self.height * 4) as usize) }
    }
}

impl Drop for Surface {
    fn drop(&mut self) {
        // SAFETY: Restore the original object, then delete exactly what `new` created.
        unsafe {
            SelectObject(self.dc, self.previous);
            assert!(DeleteObject(self.bitmap.into()).as_bool());
            assert!(DeleteDC(self.dc).as_bool());
        }
    }
}

/// The caption strip in physical pixels: the peers are placed at the window's device scale, so
/// every check reads the pixels where each peer actually is, at any display scale.
#[derive(Clone, Copy)]
struct Strip {
    width: i32,
    height: i32,
}

impl Strip {
    fn of(parent: HWND, scale: f64) -> Result<Self, Box<dyn std::error::Error>> {
        let mut client = RECT::default();
        // SAFETY: Read-only client bounds of this test's own window.
        unsafe { windows::Win32::UI::WindowsAndMessaging::GetClientRect(parent, &raw mut client) }?;
        Ok(Self { width: client.right, height: (f64::from(HEIGHT) * scale).ceil() as i32 })
    }
}

/// Peer `id`'s rectangle in its parent's client pixels.
pub(super) fn peer_bounds(parent: HWND, id: u16) -> Result<RECT, Box<dyn std::error::Error>> {
    // SAFETY: The peers belong to this test's window on its own private desktop.
    let peer = unsafe { GetDlgItem(Some(parent), i32::from(id)) }?;
    let mut bounds = RECT::default();
    // SAFETY: Read-only bounds of the peer.
    unsafe { GetWindowRect(peer, &raw mut bounds) }?;
    let mut client = windows::Win32::Foundation::POINT::default();
    // SAFETY: Map this test window's client origin to the screen; no input or paint occurs.
    if !unsafe { windows::Win32::Graphics::Gdi::ClientToScreen(parent, &raw mut client) }.as_bool()
    {
        return Err("client origin unavailable".into());
    }
    Ok(RECT {
        left: bounds.left - client.x,
        top: bounds.top - client.y,
        right: bounds.right - client.x,
        bottom: bounds.bottom - client.y,
    })
}

/// Paint every visible caption peer through the owner-draw message the window would send, each
/// into its own surface, and place the results in a caption strip flattened over the dark
/// caption background as opaque BGRA. A visible peer outside the strip is an error, so no
/// check can pass or fail on pixels it never looked at.
fn paint(parent: HWND, strip: Strip) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let mut pixels = vec![0_u8; (strip.width * strip.height * 4) as usize];
    for pixel in pixels.chunks_exact_mut(4) {
        pixel.copy_from_slice(&[32, 32, 32, 255]);
    }
    for id in 1..=PEERS as u16 {
        // SAFETY: The peers belong to this test's window on its own private desktop.
        let peer = unsafe { GetDlgItem(Some(parent), i32::from(id)) }?;
        // SAFETY: Scalar visibility query.
        if !unsafe { IsWindowVisible(peer) }.as_bool() {
            continue;
        }
        let bounds = peer_bounds(parent, id)?;
        if bounds.left < 0
            || bounds.top < 0
            || bounds.right > strip.width
            || bounds.bottom > strip.height
        {
            return Err(format!("peer {id} at {bounds:?} lies outside the caption strip").into());
        }
        let (width, height) = (bounds.right - bounds.left, bounds.bottom - bounds.top);
        let surface = Surface::new(width, height)?;
        let item = DRAWITEMSTRUCT {
            CtlType: ODT_BUTTON,
            CtlID: id as u32,
            itemAction: ODA_DRAWENTIRE,
            itemState: ODS_DEFAULT,
            hwndItem: peer,
            hDC: surface.dc,
            rcItem: RECT { left: 0, top: 0, right: width, bottom: height },
            itemID: 0,
            itemData: 0,
        };
        // SAFETY: The structure outlives this synchronous same-thread message.
        unsafe {
            SendMessageW(
                parent,
                WM_DRAWITEM,
                Some(WPARAM(id as usize)),
                Some(LPARAM(&raw const item as isize)),
            );
        }
        for (row, line) in surface.pixels().chunks_exact(width as usize * 4).enumerate() {
            for (column, bgra) in line.chunks_exact(4).enumerate() {
                let (x, y) = (bounds.left + column as i32, bounds.top + row as i32);
                let at = ((y * strip.width + x) * 4) as usize;
                let inverse = 255 - u32::from(bgra[3]);
                for channel in 0..3 {
                    let below = u32::from(pixels[at + channel]);
                    pixels[at + channel] = (u32::from(bgra[channel]) + below * inverse / 255) as u8;
                }
            }
        }
    }
    Ok(pixels)
}

/// Strip pixels inside `bounds` whose BGR channels satisfy `matches`.
fn count(pixels: &[u8], strip: Strip, bounds: RECT, matches: impl Fn(&[u8]) -> bool) -> usize {
    let columns = bounds.left.max(0)..bounds.right.min(strip.width);
    (bounds.top.max(0)..bounds.bottom.min(strip.height))
        .flat_map(|y| columns.clone().map(move |x| ((y * strip.width + x) * 4) as usize))
        .filter(|at| matches(&pixels[*at..*at + 3]))
        .count()
}

/// Pixels in the update button's area that carry `tint`.
fn tinted(pixels: &[u8], strip: Strip, bounds: RECT, tint: [u8; 3]) -> usize {
    count(pixels, strip, bounds, |bgr| bgr == [tint[2], tint[1], tint[0]])
}

fn caption_body() -> TestResult {
    let identity = identity()?;
    let mut builder = EventLoopBuilder::<NativeAction>::with_user_event();
    tao::platform::windows::EventLoopBuilderExtWindows::with_any_thread(&mut builder, true);
    let events = builder.build();
    let window = Rc::new(
        WindowBuilder::new()
            .with_title("Main caption qualification")
            .with_inner_size(LogicalSize::new(f64::from(WIDTH), 600.0))
            .with_decorations(false)
            .with_visible(true)
            .build(&events)?,
    );
    let parent = HWND(window.hwnd() as *mut c_void);
    let retry = Recovery::new(identity)
        .presentation(MainRecoveryLabels { retry: "Retry", retry_ready: "Retry ready" });
    let measure = |text: &str| crate::windows_callbacks::caption::measure_text(parent, text);
    let words = messages("en");
    let initial = CaptionUpdates::default().view(words);
    let first = caption_controls(f64::from(WIDTH), false, LABELS, retry, &initial, measure)?;
    let mut controls = Controls::new(
        window.clone(),
        events.create_proxy(),
        &Specification::buttons(&first),
        PaintMode::NativeButtons(identity),
        None,
    )?;
    controls.configure_caption(true, crate::NativeMaterial::ThemedFallback);
    controls.initialize()?;
    let strip = Strip::of(parent, window.scale_factor())?;
    let mut captures = Vec::new();
    let mut problems = Vec::new();
    let result = (|| -> TestResult {
        for state in states() {
            let view = state.caption.view(words);
            let layout = caption_controls(f64::from(WIDTH), false, LABELS, retry, &view, measure)?;
            controls.present_caption(&layout, &view)?;
            for id in 5..=PEERS as u16 {
                // SAFETY: Scalar lookups on this test's own window.
                let visible =
                    unsafe { IsWindowVisible(GetDlgItem(Some(parent), i32::from(id))?).as_bool() };
                assert_eq!(visible, state.shown.contains(&id), "{}: peer {id}", state.name);
            }
            let pixels = paint(parent, strip)?;
            let update = peer_bounds(parent, crate::main_updates::UPDATE)?;
            let ready = tinted(
                &pixels,
                strip,
                update,
                crate::windows_callbacks::caption::update_tint(false),
            );
            let warning = tinted(
                &pixels,
                strip,
                update,
                crate::windows_callbacks::caption::update_tint(true),
            );
            let tints_right = match state.name {
                "caption-update-warning" => warning > 20 && ready == 0,
                _ if state.shown.contains(&5) => ready > 20 && warning == 0,
                _ => (ready, warning) == (0, 0),
            };
            if !tints_right {
                problems.push(format!("{}: ready {ready}, warning {warning}", state.name));
            }
            for id in [6_u16, 7, 8, 9] {
                if state.shown.contains(&id) {
                    let bounds = peer_bounds(parent, id)?;
                    let bright = count(&pixels, strip, bounds, |bgr| bgr[0] > 180 && bgr[1] > 180);
                    if bright <= 20 {
                        problems.push(format!("{}: peer {id} shows no words", state.name));
                    }
                }
            }
            captures.push((state.name, strip.width as u32, strip.height as u32, pixels));
        }
        Ok(())
    })();
    let shutdown = controls.shutdown();
    assert!(controls.retired());
    drop(controls);
    drop(window);
    result?;
    shutdown?;
    crate::report::surface::native_tests::evidence::keep(&captures)?;
    assert!(problems.is_empty(), "{problems:#?}");
    Ok(())
}

#[test]
fn every_update_caption_state_shows_on_the_real_caption_peers() -> TestResult {
    crate::windows_test_desktop::run(
        concat!(module_path!(), "::every_update_caption_state_shows_on_the_real_caption_peers"),
        caption_body,
    )
}

#[test]
fn caption_retry_keeps_its_own_state_alongside_updates() -> TestResult {
    let identity = identity()?;
    let mut recovery = Recovery::new(identity);
    let lease = sugarsubstitute_contracts::lifecycle::LeaseKey {
        run: identity.run,
        resource: sugarsubstitute_contracts::lifecycle::ResourceKey {
            owner: identity.run.owner,
            id: sugarsubstitute_contracts::lifecycle::ResourceId::new(1)?,
            generation: sugarsubstitute_contracts::lifecycle::ResourceGeneration::new(1)?,
        },
        connection: sugarsubstitute_contracts::lifecycle::ConnectionId::new(1)?,
        id: sugarsubstitute_contracts::lifecycle::LeaseId::new(1)?,
    };
    recovery.present(identity, lease, MainBackendState::RetryReady)?;
    let mut caption = CaptionUpdates::default();
    caption.observe(UpdateFeed {
        glyph: crate::main_updates::UpdateGlyph::Ready,
        ..UpdateFeed::default()
    });
    let view = caption.view(messages("en"));
    let measure = |text: &str| Ok::<_, std::convert::Infallible>(text.len() as f64 * 6.0);
    let layout = caption_controls(
        1280.0,
        false,
        LABELS,
        recovery.presentation(MainRecoveryLabels { retry: "Retry", retry_ready: "Ready" }),
        &view,
        measure,
    )?;
    assert!(layout[3].visible && layout[4].visible);
    Ok(())
}
