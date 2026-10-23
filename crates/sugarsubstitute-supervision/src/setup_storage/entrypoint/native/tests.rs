// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Verify actual shell-link launch properties before reusing retained setup output.
use super::*;
use crate::setup_storage::test_worker::{self as worker, TestResult};
use windows::Win32::UI::{Shell::SLDF_RUNAS_USER, WindowsAndMessaging::SW_SHOWMINNOACTIVE};

#[test]
fn custom_arguments_working_directory_visibility_and_elevation_are_preserved_and_rejected()
-> TestResult {
    worker::run(|_, _| {
        let temporary = tempfile::tempdir()?;
        let executable = temporary.path().join("sugarsubstitute.exe");
        std::fs::write(&executable, b"inert, never executed")?;
        for mode in 0..4 {
            let path = temporary.path().join(format!("custom-{mode}.lnk"));
            create(&path, &executable, temporary.path())?;
            assert_eq!(target(&path)?, executable);
            let _apartment = Apartment::enter()?;
            // SAFETY: The fixture's local COM interfaces and strings outlive synchronous
            // calls and retire on this same worker before its apartment guard.
            unsafe {
                let link: IShellLinkW = CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER)?;
                let storage: IPersistFile = link.cast()?;
                storage.Load(&HSTRING::from(path.as_os_str()), STGM_READ)?;
                match mode {
                    0 => link.SetArguments(&HSTRING::from("--user-argument"))?,
                    1 => link.SetWorkingDirectory(&HSTRING::from(
                        temporary.path().join("other").as_os_str(),
                    ))?,
                    2 => link.SetShowCmd(SW_SHOWMINNOACTIVE)?,
                    _ => {
                        let data: IShellLinkDataList = link.cast()?;
                        data.SetFlags(data.GetFlags()? | SLDF_RUNAS_USER.0 as u32)?;
                    }
                }
                storage.Save(&HSTRING::from(path.as_os_str()), true)?;
            }
            let before = std::fs::read(&path)?;
            assert!(target(&path).is_err(), "custom launch mode {mode}");
            assert_eq!(std::fs::read(&path)?, before);
        }
        Ok(())
    })
}

#[test]
fn expandable_icon_metadata_keeps_the_absolute_launch_target_and_original_link_bytes() -> TestResult
{
    worker::run(|_, _| {
        let temporary = tempfile::tempdir()?;
        let executable = temporary.path().join("sugarsubstitute.exe");
        std::fs::write(&executable, b"inert, never executed")?;
        let path = temporary.path().join("owned.lnk");
        create(&path, &executable, temporary.path())?;
        {
            let _apartment = Apartment::enter()?;
            // SAFETY: These local COM interfaces and strings remain live through synchronous
            // native calls and retire before the same-thread apartment guard.
            unsafe {
                let link: IShellLinkW = CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER)?;
                let storage: IPersistFile = link.cast()?;
                storage.Load(&HSTRING::from(path.as_os_str()), STGM_READ)?;
                link.SetIconLocation(&HSTRING::from(r"%SystemRoot%\System32\shell32.dll"), 0)?;
                let data: IShellLinkDataList = link.cast()?;
                data.SetFlags(data.GetFlags()? | SLDF_HAS_EXP_ICON_SZ.0 as u32)?;
                storage.Save(&HSTRING::from(path.as_os_str()), true)?;
                storage.Load(&HSTRING::from(path.as_os_str()), STGM_READ)?;
                assert_ne!(data.GetFlags()? & SLDF_HAS_EXP_ICON_SZ.0 as u32, 0);
            }
        }
        let bytes = std::fs::read(&path)?;
        assert_eq!(target(&path)?, executable);
        assert_eq!(std::fs::read(&path)?, bytes);
        Ok(())
    })
}
