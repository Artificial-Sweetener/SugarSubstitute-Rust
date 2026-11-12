// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Qualify real installer output through disclosure, append, collapse, reopen and checked teardown.
use super::*;
use sugarsubstitute_application::setup_flow::{Completed, Location, Outcome, RuntimeMode};
use sugarsubstitute_contracts::presentation::PreparedFrame;
use sugarsubstitute_desktop::{ConsoleText, ConsoleView, pixel_capture::PixelCapture};

#[derive(Default)]
struct Evidence {
    finished: bool,
    transcripts: usize,
    glyphs: bool,
}
impl PixelCapture for Evidence {
    fn surface(&mut self, _: &PreparedFrame) {
        self.transcripts = 0;
        self.glyphs = false;
    }
    fn overlay(
        &mut self,
        control: Option<u16>,
        _: [i32; 4],
        _: Option<[i32; 4]>,
        _: usize,
        pixels: &[u8],
    ) {
        if control == Some(ConsoleView::CONTROL_ID) {
            self.transcripts += 1;
            self.glyphs |=
                pixels.chunks_exact(4).any(|pixel| pixel[..3].iter().all(|channel| *channel > 100));
        }
    }
    fn finished(&mut self) {
        self.finished = true;
    }
}
struct ConsoleFixture {
    owner: Native,
    phase: u8,
    evidence: Evidence,
    finished: bool,
}
impl Lifetime for ConsoleFixture {
    fn inventory(&self) -> &[Control] {
        self.owner.inventory()
    }
    fn picker(&mut self) -> Option<native::Picker> {
        self.owner.picker()
    }
    fn console(&self) -> Option<sugarsubstitute_desktop::installer::InstallerConsole<'_>> {
        self.owner.console()
    }
    fn start(&mut self, wake: Arc<dyn CompletionWake>) -> Result<()> {
        seed_progress(&mut self.owner)?;
        super::start(&mut self.owner, wake)?;
        let output = self.owner.controller.console.as_mut().ok_or("Console owner absent")?;
        for index in 1..=80 {
            output
                .append(&format!("Provisioning record {index}: 日本語, Español, 한국어, 中文"))?;
        }
        Ok(())
    }
    fn resize(&mut self, shape: FrameShape) -> Result<()> {
        self.owner.resize(shape)
    }
    fn input(&mut self, event: native::Event) -> Result<()> {
        self.owner.input(event)
    }
    fn advance(&mut self, elapsed: Duration) -> Result<()> {
        if std::mem::take(&mut self.evidence.finished) {
            let visible = self.phase != 2;
            assert_eq!(self.evidence.transcripts, usize::from(visible));
            assert_eq!(self.evidence.glyphs, visible);
            let output = self.owner.controller.console.as_mut().ok_or("Console owner absent")?;
            assert_eq!(output.len(), if self.phase == 3 { 81 } else { 80 });
            if self.phase == 2 {
                output.append("Final record appended while the console was collapsed")?;
            }
            self.phase += 1;
            if self.phase < 4 {
                self.toggle()?;
            }
        }
        self.owner.advance(elapsed)
    }
    fn page(&self) -> Option<native::Page<'_>> {
        self.owner.page()
    }
    fn presented(&mut self) -> Result<()> {
        if self.phase == 0 {
            self.phase = 1;
            self.toggle()?;
        }
        Ok(())
    }
    fn pixel_capture(&mut self) -> Option<&mut dyn PixelCapture> {
        let ready = self.owner.page().is_some()
            && self
                .owner
                .console()
                .is_some_and(|console| console.view.visible == (self.phase != 2))
            && (1..=3).contains(&self.phase);
        ready.then_some(&mut self.evidence)
    }
    fn close_requested(&self) -> bool {
        self.phase == 4
    }
    fn close(&mut self) -> Result<()> {
        self.owner.close()
    }
    fn settled(&mut self) -> Result<bool> {
        self.owner.settled()
    }
    fn finish(&mut self) -> Result<()> {
        self.owner.finish()?;
        assert!(self.owner.controller.owner.is_none());
        assert!(self.owner.controller.console.is_none());
        self.finished = true;
        Ok(())
    }
}
impl ConsoleFixture {
    fn toggle(&mut self) -> Result<()> {
        self.owner.input(native::Event::Control(Action::Details.id(), Input::Invoke))
    }
}
/// Supply installation outcomes at the external boundary without writing an installation.
fn seed_progress(native: &mut Native) -> Result<()> {
    let flow = &mut native.session.navigation.flow;
    for result in [
        Completed::Locale,
        Completed::Location(Location {
            comfy_root: "ComfyUI".into(),
            models: "Models".into(),
            outputs: "Outputs".into(),
            existing: None,
        }),
    ] {
        let request = flow.primary(Duration::ZERO)?.ok_or("Expected request absent")?;
        flow.receive(request.id, Outcome::Completed(result), Duration::ZERO)?;
    }
    flow.set_mode(RuntimeMode::Attached)?;
    assert!(flow.primary(Duration::ZERO)?.is_none());
    for _ in 0..2 {
        let request = flow.primary(Duration::ZERO)?.ok_or("Expected validation absent")?;
        if let Some(install) =
            flow.receive(request.id, Outcome::Completed(Completed::Validated), Duration::ZERO)?
        {
            flow.progress(install.id, "Installing dependencies".into(), Some(42.0))?;
        }
    }
    assert_eq!(flow.page(), Page::Progress);
    Ok(())
}
fn qualify() -> Result<()> {
    let mut fixture = ConsoleFixture {
        owner: Native::new(PathBuf::new(), PathBuf::new(), &[], "Test installation".into())?,
        phase: 0,
        evidence: Evidence::default(),
        finished: false,
    };
    native::run(
        native::Window::installer(
            "Native installer console qualification".into(),
            true,
            Action::Close.id(),
        ),
        &mut fixture,
    )?;
    assert_eq!(fixture.phase, 4);
    assert!(fixture.finished);
    Ok(())
}
#[test]
fn native_installer_console_retains_output_through_disclosure_and_retires_execution() -> Result<()>
{
    sugarsubstitute_desktop::qualify_installer_test(
        concat!(
            module_path!(),
            "::native_installer_console_retains_output_through_disclosure_and_retires_execution"
        ),
        qualify,
    )
}
