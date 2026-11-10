// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! An image Windows cannot run is a typed creation error, never a blocking hard-error dialog.
use super::*;
use crate::creation_error_mode::{is_image_format_error, thread_mode};
use std::io::Read;

/// Bytes the loader refuses: nothing, text, a bare DOS signature and a truncated real image.
fn unrunnable_images() -> Result<Vec<(&'static str, Vec<u8>)>, std::io::Error> {
    let mut truncated = Vec::new();
    std::fs::File::open(std::env::current_exe()?)?.take(1024).read_to_end(&mut truncated)?;
    Ok(vec![
        ("empty", Vec::new()),
        ("text", b"inert, never executed".to_vec()),
        ("dos_signature", b"MZ inert, never executed".to_vec()),
        ("truncated_executable", truncated),
    ])
}

#[test]
fn an_unrunnable_image_fails_creation_without_a_dialog_or_a_process() -> TestResult {
    worker::run_reserved(CREATION_RESERVATION_BYTES, |context, _| {
        let root = tempfile::tempdir()?;
        for (kind, bytes) in unrunnable_images()? {
            let image = root.path().join(format!("{kind}.exe"));
            std::fs::write(&image, bytes)?;
            let launch =
                ApplicationLaunch::new(WindowsLaunchCommand::from_os(&image, root.path(), &[])?);
            let mode = thread_mode();
            let cause = launch.launch(context).err().ok_or("Unrunnable image started")?;
            assert_eq!(cause.operation(), "application_launch_create", "{kind}");
            assert!(is_image_format_error(cause.win32_code()), "{kind}: {:?}", cause.win32_code());
            assert_eq!(thread_mode(), mode, "{kind}");
            assert_eq!(
                crate::native_claims::lock(&launch.0).creation,
                CreationOutcome::NotCreated,
                "{kind}"
            );
        }
        Ok(())
    })
}
