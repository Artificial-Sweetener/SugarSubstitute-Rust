// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Share the genuine returned failure without copying its guarded input or read frame.
use super::*;
use std::{
    fmt,
    sync::{Arc, Mutex},
};
struct FailedMain {
    cause: worker::MainError,
    frame: Mutex<Option<ReceivedLifecycleFrame>>,
    // Original192KiB input credit covers this bounded control alongside the retained path.
    _input: PreparedMainInput,
}
/// A closed shared failure created on the admitted worker; clones allocate no shared shell.
pub(super) struct MainPreparationFailure(Option<Arc<FailedMain>>);
impl MainPreparationFailure {
    pub(super) fn new(
        cause: worker::MainError,
        frame: Option<ReceivedLifecycleFrame>,
        input: PreparedMainInput,
    ) -> Self {
        Self(Some(Arc::new(FailedMain { cause, frame: Mutex::new(frame), _input: input })))
    }
    /// A busy/poisoned original guard blocks native absence rather than claiming empty custody.
    pub(super) fn frame_pending(&self) -> bool {
        self.0
            .as_ref()
            .is_some_and(|failure| failure.frame.try_lock().map_or(true, |frame| frame.is_some()))
    }
    pub(super) fn release_frame(&self) -> Result<()> {
        if let Some(failure) = &self.0 {
            let frame = failure
                .frame
                .lock()
                .map_err(|_| "Original main failure frame custody poisoned")?
                .take();
            drop(frame);
        }
        Ok(())
    }
}
impl Clone for MainPreparationFailure {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}
impl Drop for MainPreparationFailure {
    fn drop(&mut self) {
        // No Weak or raw Arc escapes. Last header frees before data and its original credit.
        if let Some(failure) = self.0.take() {
            drop(Arc::into_inner(failure));
        }
    }
}
impl fmt::Debug for MainPreparationFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("MainPreparationFailure { retained }")
    }
}
impl fmt::Display for MainPreparationFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Main request construction failed with retained original source")
    }
}
impl Error for MainPreparationFailure {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        self.0.as_ref().map(|failure| &failure.cause as &dyn Error)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};
    use sugarsubstitute_contracts::{
        configuration::{LoopbackOrigin, Presentation, Theme},
        coordinator_entry::{
            EntryConnectionView, EntryEnvironmentView, EntryFrontendView, EntryLaunchRequestView,
            EntryWindowsPathView,
        },
    };
    struct Release(Arc<AtomicU64>);
    impl MemoryRelease for Release {
        fn release(&self, bytes: u64) {
            self.0.fetch_add(bytes, Ordering::AcqRel);
        }
    }
    #[test]
    fn original_typed_main_failure_and_input_credit_survive_shared_aliases() -> Result<()> {
        let bytes: Vec<_> =
            "E:\\requested-profile".encode_utf16().flat_map(u16::to_le_bytes).collect();
        let path = EntryWindowsPathView::from_le_bytes(&bytes)?;
        let view = EntryLaunchRequestView::new(
            EntryConnectionView::Attached(LoopbackOrigin::parse("http://127.0.0.1:8188", 8188)?),
            EntryFrontendView::new(path, path, path, path),
            path,
            "Profile_1",
            Presentation::new("en", Theme::Light, false)?,
            EntryEnvironmentView::from_le_bytes(&[0, 0, 0, 0])?,
        )?;
        let released = Arc::new(AtomicU64::new(0));
        let memory =
            MemoryLease::new(configuration::RESERVATION_BYTES, Box::new(Release(released.clone())));
        let input = configuration::project_main(view, &memory)?;
        let cause = MainPreparationFailure::new(worker::MainError::Cancelled, None, input);
        let alias = cause.clone();
        assert!(matches!(
            alias.source().and_then(|source| source.downcast_ref::<worker::MainError>()),
            Some(worker::MainError::Cancelled)
        ));
        cause.release_frame()?;
        drop((cause, memory));
        assert_eq!(released.load(Ordering::Acquire), 0);
        assert_eq!(
            alias.source().ok_or("Original cause lost")?.to_string(),
            "Original main request was cancelled"
        );
        drop(alias);
        assert_eq!(released.load(Ordering::Acquire), configuration::RESERVATION_BYTES);
        Ok(())
    }
}
