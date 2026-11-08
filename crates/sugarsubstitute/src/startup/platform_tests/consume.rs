// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise failure stages through the public startup polling boundary and real execution owner.

use super::*;
use sugarsubstitute_desktop::{StartupConsumeFailure, StartupConsumeStage};

struct Wake;
impl CompletionWake for Wake {
    fn wake(&self) -> Result<(), ExecutionError> {
        Ok(())
    }
}

/// Start the genuine host while the external platform adapter owns no native resources.
pub(in crate::startup) fn started_host() -> BackendResult<StartupHost<Platform>> {
    let mut host = host()?;
    host.child.start_allowed = true;
    host.start(Arc::new(Wake))?;
    Ok(host)
}

/// Settle the original scope and native absence through the public shutdown contract.
pub(in crate::startup) fn finish(host: &mut StartupHost<Platform>) -> BackendResult<()> {
    host.request_shutdown()?;
    host.observe(DesktopStartupObservation::NativeRetired(host.run_key()))?;
    let guard = Instant::now() + Duration::from_secs(10);
    while !host.poll_shutdown()? {
        if Instant::now() >= guard {
            return Err("Startup test execution did not settle".into());
        }
        std::hint::spin_loop();
    }
    host.finish_shutdown()
}

#[test]
fn poll_frame_names_reload_failure_and_keeps_the_exact_original_box() -> BackendResult<()> {
    let mut host = started_host()?;
    host.startup = Some(Prepared {
        session: StartupSession::new(
            LaunchId::new(1)?,
            Generation::new(1)?,
            StartupConfiguration::new(
                ConnectionConfiguration::Attach(LoopbackOrigin::parse(
                    "http://127.0.0.1:8188",
                    8188,
                )?),
                ProfileId::new("poll-test".into())?,
                Presentation::new("en", Theme::Dark, true)?,
            ),
        ),
        released: false,
    });
    let original = Box::new(std::io::Error::from(std::io::ErrorKind::PermissionDenied));
    let address = original.as_ref() as *const std::io::Error;
    host.child.reload_failure = Some(original);
    let error = host.poll_frame(NonZeroUsize::MIN).err().ok_or("Reload failure absent")?;
    let retained = error.downcast_ref::<StartupConsumeFailure>().ok_or("Reload stage absent")?;
    assert_eq!(retained.stage(), StartupConsumeStage::FrontendReload);
    assert!(std::ptr::eq(
        address,
        retained.original().downcast_ref::<std::io::Error>().ok_or("Original Box absent")?,
    ));
    assert!(host.poll_frame(NonZeroUsize::MIN)?.is_none());
    finish(&mut host)
}

#[test]
fn publication_guard_names_missing_appearance_and_preserves_present_metadata() -> BackendResult<()>
{
    let error = consume_diagnostic::published_appearance::<Platform>(None)
        .err()
        .ok_or("Publication failure absent")?;
    let retained =
        error.downcast_ref::<StartupConsumeFailure>().ok_or("Publication stage absent")?;
    assert_eq!(retained.stage(), StartupConsumeStage::AppearancePublication);
    assert_eq!(retained.code(), StartupCauseCode::Opaque);
    let appearance = sugarsubstitute_splash::Appearance::demo();
    assert_eq!(
        consume_diagnostic::published_appearance::<Platform>(Some(appearance))?.reveal,
        appearance.reveal,
    );
    Ok(())
}
