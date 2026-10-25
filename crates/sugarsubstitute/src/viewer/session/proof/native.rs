// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Qualify the ordinary native window and retirement path with the same bounded proof driver.
use super::*;
use sugarsubstitute_desktop::canvas::{CpuFrame, GpuPresentation, WindowTarget};
use sugarsubstitute_image_ops::Extent;

impl Lifetime for Proof {
    fn attach_window(&mut self, target: WindowTarget) -> Result<(), Box<dyn Error>> {
        self.session.attach_window(target)
    }
    fn take_gpu_presentation(&mut self) -> Option<GpuPresentation> {
        self.session.take_gpu_presentation()
    }
    fn did_present_gpu(&mut self, target: FrameStamp) -> Result<(), Box<dyn Error>> {
        self.session.did_present_gpu(target)
    }
    fn expose(&mut self) {
        self.session.expose();
    }
    fn start(&mut self, wake: Arc<dyn CompletionWake>) -> Result<(), Box<dyn Error>> {
        self.session.start(wake)
    }
    fn resize(&mut self, extent: Extent) -> Result<(), Box<dyn Error>> {
        self.session.resize(extent)
    }
    fn input(&mut self, input: Input) -> Result<(), Box<dyn Error>> {
        self.session.input(input)
    }
    fn take_picker_request(&mut self) -> bool {
        false
    }
    fn selected(
        &mut self,
        selection: Result<Option<PathBuf>, Box<dyn Error>>,
    ) -> Result<(), Box<dyn Error>> {
        self.session.selected(selection)
    }
    fn poll(&mut self, _: Duration) -> Result<(), Box<dyn Error>> {
        if self.session.closing {
            self.session.poll(Duration::ZERO)
        } else {
            self.activity(NonZeroUsize::MIN).map(|_| ())
        }
    }
    fn frame(&self) -> Option<CpuFrame<'_>> {
        self.session.frame()
    }
    fn title(&self) -> &str {
        self.session.title()
    }
    fn next_deadline(&self) -> Option<Instant> {
        HeadlessLifetime::next_deadline(self)
    }
    fn is_closing(&self) -> bool {
        self.session.closing || self.complete
    }
    fn close(&mut self) -> Result<(), Box<dyn Error>> {
        self.request_shutdown()
    }
    fn settled(&mut self) -> Result<bool, Box<dyn Error>> {
        self.poll_shutdown()
    }
    fn finish(&mut self) -> Result<(), Box<dyn Error>> {
        self.finish_shutdown()
    }
}
