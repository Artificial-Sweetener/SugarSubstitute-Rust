// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Show each report kind in the real form window on a private desktop: capture the frames it
//! presents (renderer output and native overlays, through the host's pixel capture, never the
//! screen), drive its native peers, and prove Escape is handled while a large report is still
//! preparing. Set `SUGARSUBSTITUTE_REPORT_PROOF_DIR` to keep the captures as bitmaps. Copy is
//! not exercised: the private desktop shares the interactive window station's clipboard.
use super::*;
use ::windows::{
    Win32::{
        Foundation::{HWND, LPARAM, WPARAM},
        UI::{
            Input::KeyboardAndMouse::VK_ESCAPE,
            WindowsAndMessaging::{
                BM_CLICK, FindWindowW, GetDlgItem, IsWindowVisible, PostMessageW, WM_KEYDOWN,
            },
        },
    },
    core::{HSTRING, PCWSTR},
};
use std::{cell::RefCell, rc::Rc};
use sugarsubstitute_contracts::presentation::{FrameShape, PreparedFrame};

/// One kept capture: name, width, height and opaque BGRA pixels.
type Capture = (&'static str, u32, u32, Vec<u8>);

/// Captures requested by the script and taken by the host after its next presentation, and
/// whether the report had nothing left to prepare after the last turn.
#[derive(Default)]
struct Captures {
    request: Option<&'static str>,
    taken: Vec<Capture>,
    idle: bool,
}

/// Composite the presented frame and its native overlays, then flatten over the fallback
/// material color the window shows where its pixels are transparent.
struct Observer {
    shared: Rc<RefCell<Captures>>,
    width: usize,
    height: usize,
    pixels: Vec<u8>,
}

impl crate::pixel_capture::PixelCapture for Observer {
    fn surface(&mut self, frame: &PreparedFrame) {
        self.width = frame.shape().width() as usize;
        self.height = frame.shape().height() as usize;
        self.pixels = frame.pixels().to_vec();
    }
    fn overlay(
        &mut self,
        _: Option<u16>,
        bounds: [i32; 4],
        clip: Option<[i32; 4]>,
        stride: usize,
        pixels: &[u8],
    ) {
        let [left, top, right, bottom] = clip.unwrap_or(bounds);
        for y in top.max(bounds[1]).max(0)..bottom.min(bounds[3]).min(self.height as i32) {
            for x in left.max(bounds[0]).max(0)..right.min(bounds[2]).min(self.width as i32) {
                let source = (y - bounds[1]) as usize * stride + (x - bounds[0]) as usize * 4;
                let Some(bgra) = pixels.get(source..source + 4) else { continue };
                let target = (y as usize * self.width + x as usize) * 4;
                let inverse = 255 - u32::from(bgra[3]);
                for (channel, value) in [bgra[2], bgra[1], bgra[0], bgra[3]].into_iter().enumerate()
                {
                    let below = u32::from(self.pixels[target + channel]);
                    self.pixels[target + channel] =
                        (u32::from(value) + below * inverse / 255) as u8;
                }
            }
        }
    }
    fn finished(&mut self) {
        let mut shared = self.shared.borrow_mut();
        let Some(name) = shared.request.take() else { return };
        let corner = (self.height.saturating_sub(2) * self.width + 2) * 4;
        let background: u32 =
            if self.pixels.get(corner).is_some_and(|v| *v < 128) { 32 } else { 243 };
        let mut bgra = Vec::with_capacity(self.pixels.len());
        for rgba in self.pixels.chunks_exact(4) {
            let flat = |value: u8| {
                (u32::from(value) + background * (255 - u32::from(rgba[3])) / 255) as u8
            };
            bgra.extend_from_slice(&[flat(rgba[2]), flat(rgba[1]), flat(rgba[0]), 255]);
        }
        shared.taken.push((name, self.width as u32, self.height as u32, bgra));
    }
}
use sugarsubstitute_contracts::{
    instance_conflict::{ConflictAction, ConflictController, ConflictPhase, ConflictView},
    localization::messages,
};

type TestResult = std::result::Result<(), Box<dyn Error>>;

/// Consecutive idle turns (shown, nothing left to prepare) before each scripted step.
const SETTLE: usize = 3;

#[derive(Clone, Copy, Debug)]
enum Step {
    Capture(&'static str),
    Click(ReportAction),
    Escape,
}

/// Wrap a real owner and act on the window between its turns.
struct Script<'a> {
    title: &'static str,
    inner: &'a mut dyn ReportOwner,
    steps: Vec<Step>,
    next: usize,
    settled: usize,
    polls: usize,
    escaped_at: Option<usize>,
    escaped_while_busy: bool,
    closed_at: Option<usize>,
    captures: Rc<RefCell<Captures>>,
    failure: Option<String>,
}

impl<'a> Script<'a> {
    fn new(title: &'static str, inner: &'a mut dyn ReportOwner, steps: Vec<Step>) -> Self {
        Self {
            title,
            inner,
            steps,
            next: 0,
            settled: 0,
            polls: 0,
            escaped_at: None,
            escaped_while_busy: false,
            closed_at: None,
            captures: Rc::default(),
            failure: None,
        }
    }

    fn window(&self) -> Option<HWND> {
        // SAFETY: Read-only lookup on this test's private desktop, where it owns the only window.
        let hwnd = unsafe { FindWindowW(PCWSTR::null(), &HSTRING::from(self.title)) }.ok()?;
        // SAFETY: Scalar visibility query of the window found above.
        unsafe { IsWindowVisible(hwnd) }.as_bool().then_some(hwnd)
    }

    fn act(&mut self, hwnd: HWND, step: Step) -> TestResult {
        match step {
            Step::Capture(name) => self.captures.borrow_mut().request = Some(name),
            Step::Click(action) => {
                // SAFETY: The peer belongs to this test's window; BM_CLICK carries no pointers.
                unsafe {
                    let peer = GetDlgItem(Some(hwnd), i32::from(action.id().get()))?;
                    PostMessageW(Some(peer), BM_CLICK, WPARAM(0), LPARAM(0))?;
                }
            }
            Step::Escape => {
                self.escaped_at = Some(self.polls);
                // SAFETY: Post a plain key message to this test's own window.
                unsafe {
                    PostMessageW(Some(hwnd), WM_KEYDOWN, WPARAM(VK_ESCAPE.0.into()), LPARAM(1))?;
                }
            }
        }
        Ok(())
    }
}

impl ReportOwner for Script<'_> {
    fn start(&mut self, wake: Arc<dyn CompletionWake>) -> TestResult {
        self.inner.start(wake)
    }
    fn command(&mut self, command: ReportCommand, report: &mut ReportControl<'_>) -> TestResult {
        self.inner.command(command, report)
    }
    fn poll(&mut self, report: &mut ReportControl<'_>) -> TestResult {
        self.polls += 1;
        self.inner.poll(report)?;
        let idle = self.captures.borrow().idle;
        let step = self.steps.get(self.next).copied();
        // Escape is sent as soon as the window shows, to prove it is handled mid-preparation.
        let urgent = matches!(step, Some(Step::Escape));
        if let Some(hwnd) = self.window().filter(|_| idle || urgent) {
            self.settled += 1;
            if (self.settled >= SETTLE || urgent)
                && let Some(step) = step
            {
                if urgent {
                    self.escaped_while_busy = !idle;
                }
                self.next += 1;
                self.settled = 0;
                if let Err(cause) = self.act(hwnd, step) {
                    self.failure = Some(cause.to_string());
                    report.close();
                }
            }
        }
        Ok(())
    }
    fn close(&mut self) -> TestResult {
        self.closed_at.get_or_insert(self.polls);
        self.inner.close()
    }
    fn settled(&mut self) -> std::result::Result<bool, Box<dyn Error>> {
        self.inner.settled()
    }
}

/// The real report lifetime, failing at once when the window would otherwise wait forever:
/// no frame of the window's size exists and no work is pending to produce one.
struct Observed<'a> {
    inner: lifetime::ReportLifetime<'a>,
    window: Option<FrameShape>,
    /// The preferred size the window had applied when it last reported its size.
    acknowledged: Option<[u32; 2]>,
    closing: bool,
    observer: Observer,
}

impl crate::installer::native::Lifetime for Observed<'_> {
    fn inventory(&self) -> &[sugarsubstitute_ui::form::Control] {
        self.inner.inventory()
    }
    fn picker(&mut self) -> Option<crate::installer::native::Picker> {
        self.inner.picker()
    }
    fn console(&self) -> Option<crate::installer::InstallerConsole<'_>> {
        None
    }
    fn work_area(&mut self, height: f64) {
        self.inner.work_area(height);
    }
    fn start(&mut self, wake: Arc<dyn CompletionWake>) -> TestResult {
        self.inner.start(wake)
    }
    fn resize(&mut self, shape: FrameShape) -> TestResult {
        self.window = Some(shape);
        self.acknowledged = self.inner.preferred_size();
        self.inner.resize(shape)
    }
    fn input(&mut self, event: crate::installer::native::Event) -> TestResult {
        self.inner.input(event)
    }
    fn advance(&mut self, elapsed: std::time::Duration) -> TestResult {
        self.inner.advance(elapsed)?;
        let presentable =
            self.inner.page().is_some_and(|page| Some(page.frame.shape()) == self.window);
        let pending = self.inner.pending();
        self.observer.shared.borrow_mut().idle = presentable && !pending;
        // A request the window has not applied yet is still on its way.
        let applied = self.acknowledged == self.inner.preferred_size();
        if !self.closing && applied && !pending && !presentable {
            let window = self.window.map(|w| (w.width(), w.height()));
            let frame =
                self.inner.page().map(|p| (p.frame.shape().width(), p.frame.shape().height()));
            return Err(format!(
                "report window {window:?} has no matching frame ({frame:?}) and no work pending"
            )
            .into());
        }
        Ok(())
    }
    fn page(&self) -> Option<crate::installer::native::Page<'_>> {
        self.inner.page()
    }
    fn presented(&mut self) -> TestResult {
        self.inner.presented()
    }
    fn preferred_size(&self) -> Option<[u32; 2]> {
        self.inner.preferred_size()
    }
    fn clipboard(&mut self) -> Option<String> {
        self.inner.clipboard()
    }
    fn copied(&mut self, result: std::result::Result<(), String>) -> TestResult {
        self.inner.copied(result)
    }
    fn close_requested(&self) -> bool {
        self.inner.close_requested()
    }
    fn close(&mut self) -> TestResult {
        self.closing = true;
        self.inner.close()
    }
    fn settled(&mut self) -> std::result::Result<bool, Box<dyn Error>> {
        self.inner.settled()
    }
    fn finish(&mut self) -> TestResult {
        self.inner.finish()
    }
    fn pixel_capture(&mut self) -> Option<&mut dyn crate::pixel_capture::PixelCapture> {
        let requested = self.observer.shared.borrow().request.is_some();
        requested.then_some(&mut self.observer as &mut dyn crate::pixel_capture::PixelCapture)
    }
}

/// Show `presentation` in the real window through [`Observed`], returning the captures the
/// script requested.
fn show(
    presentation: ReportPresentation,
    script: &mut Script<'_>,
) -> std::result::Result<Vec<Capture>, Box<dyn Error>> {
    let shared = script.captures.clone();
    let window = report_window(presentation.content.window_title.clone());
    let inner = lifetime::ReportLifetime::new(presentation, script)?;
    let observer = Observer { shared: shared.clone(), width: 0, height: 0, pixels: Vec::new() };
    crate::installer::native::run(
        window,
        &mut Observed { inner, window: None, acknowledged: None, closing: false, observer },
    )?;
    Ok(std::mem::take(&mut shared.borrow_mut().taken))
}

/// Keep captures as 32-bit bitmaps when a proof directory is configured.
fn keep(captures: &[Capture]) -> TestResult {
    let Some(directory) = std::env::var_os("SUGARSUBSTITUTE_REPORT_PROOF_DIR") else {
        return Ok(());
    };
    for (name, width, height, pixels) in captures {
        let mut file = Vec::with_capacity(54 + pixels.len());
        let size = 54 + pixels.len() as u32;
        file.extend_from_slice(b"BM");
        file.extend_from_slice(&size.to_le_bytes());
        file.extend_from_slice(&[0; 4]);
        file.extend_from_slice(&54u32.to_le_bytes());
        file.extend_from_slice(&40u32.to_le_bytes());
        file.extend_from_slice(&(*width as i32).to_le_bytes());
        file.extend_from_slice(&(-(*height as i32)).to_le_bytes());
        file.extend_from_slice(&1u16.to_le_bytes());
        file.extend_from_slice(&32u16.to_le_bytes());
        file.extend_from_slice(&[0; 24]);
        file.extend_from_slice(pixels);
        std::fs::write(std::path::Path::new(&directory).join(format!("{name}.bmp")), file)?;
    }
    Ok(())
}

/// A painted capture has the report's ink: dark and light pixels, not one flat color.
fn assert_painted(captures: &[Capture]) {
    for (name, width, height, pixels) in captures {
        assert!(*width >= 700 && *height >= 200, "{name}: {width}x{height}");
        let bright =
            pixels.chunks_exact(4).filter(|p| p[0] > 200 && p[1] > 200 && p[2] > 200).count();
        let dark = pixels.chunks_exact(4).filter(|p| p[0] < 60 && p[1] < 60 && p[2] < 60).count();
        assert!(bright > 500 && dark > 500, "{name}: {bright} bright, {dark} dark");
    }
}

fn report_text(lines: usize) -> String {
    (0..lines)
        .map(|line| {
            format!("caused by ({line}): the ComfyUI backend did not answer on 127.0.0.1:8188\n")
        })
        .collect()
}

fn startup_body() -> TestResult {
    let title = "SugarSubstitute";
    let mut inner = StartupReport;
    let mut script = Script::new(
        title,
        &mut inner,
        vec![
            Step::Capture("startup-failure"),
            Step::Click(ReportAction::ToggleDetails),
            Step::Capture("startup-failure-details"),
            Step::Click(ReportAction::Close),
        ],
    );
    let captures = show(
        ReportPresentation::startup_failure(
            messages("en"),
            title,
            "Startup failed",
            report_text(30),
        ),
        &mut script,
    )?;
    assert_eq!(script.failure, None);
    assert_eq!(script.next, 4, "every step ran");
    assert_eq!(captures.len(), 2);
    assert!(captures[1].2 > captures[0].2 + 200, "details grew the window");
    assert_painted(&captures);
    keep(&captures)
}

#[test]
fn startup_failure_report_presents_reveals_details_and_closes() -> TestResult {
    crate::windows_test_desktop::run(
        concat!(module_path!(), "::startup_failure_report_presents_reveals_details_and_closes"),
        startup_body,
    )
}

fn responsive_body() -> TestResult {
    let title = "SugarSubstitute";
    let mut inner = StartupReport;
    let mut script = Script::new(title, &mut inner, vec![Step::Escape]);
    let captures = show(
        ReportPresentation::startup_failure(
            messages("en"),
            title,
            "Startup failed",
            report_text(3000),
        ),
        &mut script,
    )?;
    let escaped = script.escaped_at.ok_or("Escape was never sent")?;
    let closed = script.closed_at.ok_or("the report never closed")?;
    assert!(closed - escaped <= 3, "Escape handled within a few turns: {escaped} -> {closed}");
    assert!(script.escaped_while_busy, "Escape arrived while the report was still preparing");
    assert!(captures.is_empty());
    Ok(())
}

#[test]
fn escape_closes_the_report_while_a_large_report_is_still_preparing() -> TestResult {
    crate::windows_test_desktop::run(
        concat!(
            module_path!(),
            "::escape_closes_the_report_while_a_large_report_is_still_preparing"
        ),
        responsive_body,
    )
}

/// The previous window is unresponsive at once; after "Close it and start" it keeps closing.
struct Unresponsive {
    closing: bool,
    done: bool,
}
impl ConflictController for Unresponsive {
    fn start(&mut self, _: Arc<dyn CompletionWake>) {}
    fn poll(&mut self) -> ConflictView {
        let phase = match (self.done, self.closing) {
            (true, _) => ConflictPhase::Done,
            (false, true) => ConflictPhase::Closing,
            (false, false) => ConflictPhase::AwaitingDecision,
        };
        ConflictView { phase }
    }
    fn choose(&mut self, action: ConflictAction) {
        match action {
            ConflictAction::Close => self.closing = true,
            ConflictAction::Force | ConflictAction::Cancel => self.done = true,
        }
    }
}

fn unresponsive_body() -> TestResult {
    let title = "SugarSubstitute";
    let copy = messages("en");
    let mut controller = Unresponsive { closing: false, done: false };
    let mut inner = ConflictReport::new(&mut controller, copy.conflict);
    let mut script = Script::new(
        title,
        &mut inner,
        vec![
            Step::Capture("previous-window-not-responding"),
            Step::Click(ReportAction::Close),
            Step::Capture("previous-window-closing"),
            Step::Escape,
        ],
    );
    let captures = show(ReportPresentation::unresponsive_copy(copy, title), &mut script)?;
    assert_eq!(script.failure, None);
    assert_eq!(script.next, 4);
    assert_painted(&captures);
    keep(&captures)?;
    assert!(controller.done, "Escape cancelled through the controller");
    Ok(())
}

#[test]
fn unresponsive_window_question_closes_it_and_escape_cancels() -> TestResult {
    crate::windows_test_desktop::run(
        concat!(module_path!(), "::unresponsive_window_question_closes_it_and_escape_cancels"),
        unresponsive_body,
    )
}

/// The update report's fixture export never writes; Export is not exercised natively here.
struct NoExport;
impl UpdateExport for NoExport {
    fn start(&mut self, _: Arc<dyn CompletionWake>) -> TestResult {
        Ok(())
    }
    fn begin(&mut self, _: PathBuf) -> TestResult {
        Err("fixture export".into())
    }
    fn finished(&mut self) -> Option<std::result::Result<(), Box<dyn Error>>> {
        None
    }
    fn close(&mut self) {}
    fn settled(&mut self) -> bool {
        true
    }
}

fn update_body() -> TestResult {
    let title = "SugarSubstitute";
    let copy = messages("en");
    let mut export = NoExport;
    let mut inner = UpdateReport::new(&mut export, copy.report);
    let mut script = Script::new(
        title,
        &mut inner,
        vec![
            Step::Capture("update-failure"),
            Step::Click(ReportAction::ToggleDetails),
            Step::Capture("update-failure-details"),
            Step::Click(ReportAction::Close),
        ],
    );
    let details = "SugarSubstitute 0.5.0 update failed while downloading the package.\n\
        caused by: the connection was reset after 41.2 MB of 96.0 MB\n\
        caused by: An existing connection was forcibly closed by the remote host. (os error 10054)\n"
        .to_owned();
    let captures = show(
        ReportPresentation::update_failure(copy, title, "0.5.0", "Downloading the update", details),
        &mut script,
    )?;
    assert_eq!(script.failure, None);
    assert_eq!(script.next, 4);
    assert_painted(&captures);
    keep(&captures)
}

#[test]
fn update_failure_report_presents_copies_and_closes() -> TestResult {
    crate::windows_test_desktop::run(
        concat!(module_path!(), "::update_failure_report_presents_copies_and_closes"),
        update_body,
    )
}
