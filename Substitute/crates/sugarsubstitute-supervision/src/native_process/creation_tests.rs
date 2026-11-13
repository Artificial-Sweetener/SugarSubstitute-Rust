// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Root creation reports an image Windows cannot run as its creation failure, never as a dialog.
use super::*;
use crate::{
    LaunchEnvironment, WindowsLaunchCommand, WindowsLaunchInput,
    creation_error_mode::{is_image_format_error, thread_mode},
    setup_storage::test_worker::{self as worker, TestResult},
};

#[test]
fn an_unrunnable_image_fails_the_creation_stage_without_a_dialog_or_a_member() -> TestResult {
    worker::run_reserved(4 * 1024 * 1024, |context, _| {
        let root = tempfile::tempdir()?;
        let image = root.path().join("inert.exe");
        std::fs::write(&image, b"inert, never executed")?;
        let mut input = WindowsLaunchInput::new(
            WindowsLaunchCommand::from_os(&image, root.path(), &[])?,
            LaunchEnvironment::from_snapshot_utf16(&[0, 0], &[])?,
        )?;
        let mut family = Family::allocate_owned(context.memory())?;
        WindowsPort.prepare_job_native(&mut family)?;
        let mode = thread_mode();
        let failure =
            WindowsPort.create_native(&mut family, &mut input).err().ok_or("Image started")?;
        assert_eq!(failure.stage(), "create_process");
        assert!(is_image_format_error(failure.win32_code()), "{:?}", failure.win32_code());
        assert_eq!(thread_mode(), mode);
        assert_eq!(family.count, 0);
        WindowsPort.close(&mut family)?;
        Ok(())
    })
}
