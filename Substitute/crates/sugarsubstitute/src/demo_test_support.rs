// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Synchronize qualification tests through real execution completion notifications.

use super::*;
use crate::completion_wake::Wake;
use sugarsubstitute_contracts::execution::{Moment, WaitFuture};

/// Drive a real admitted raster task and observe its typed immutable completion.
pub(super) fn prepare(
    demo: &mut SplashDemo,
    wake: &Wake,
    elapsed: Duration,
) -> Result<PreparedFrame, Box<dyn Error>> {
    demo.request_frame(FrameShape::new(558, 558, 1.0)?, elapsed)?;
    wake.until(|| demo.poll_frame(NonZeroUsize::MIN).transpose())
}

/// Start only the real execution boundary; these tests own no native window or event loop.
pub(super) fn start(demo: &mut SplashDemo) -> Result<Arc<Wake>, Box<dyn Error>> {
    let wake = Arc::new(Wake::default());
    demo.clock = Arc::new(FrozenClock);
    demo.start(wake.clone())?;
    demo.set_visible(true)?;
    Ok(wake)
}

/// Supply a fixed execution epoch; elapsed presentation samples remain explicit test inputs.
struct FrozenClock;
impl Clock for FrozenClock {
    fn now(&self) -> Moment {
        Moment::from_nanos(0)
    }
    fn sleep_until(&self, _: Moment) -> WaitFuture {
        Box::pin(std::future::pending())
    }
}

/// Observe the demo's actual two-phase shutdown without assuming a wake means workers stopped.
pub(super) fn finish(demo: &mut SplashDemo, wake: &Wake) -> Result<(), Box<dyn Error>> {
    demo.request_shutdown()?;
    wake.until(|| match demo.poll_shutdown() {
        Ok(true) => Some(Ok(())),
        Ok(false) => None,
        Err(cause) => Some(Err(cause)),
    })?;
    demo.finish_shutdown()
}
