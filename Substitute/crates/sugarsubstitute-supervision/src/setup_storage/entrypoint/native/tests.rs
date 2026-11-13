// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Verify actual shell-link launch properties before reusing retained setup output.
use super::*;
use crate::{
    durable_storage::files::Directory,
    setup_storage::test_worker::{self as worker, TestResult},
};
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
            std::fs::write(&path, encode(&executable)?)?;
            assert!(super::super::launches(&target(&path)?, &executable)?);
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

/// The shell keeps a working directory as written but stores the target in its resolved long
/// spelling, so a `TEMP` or profile reached through 8.3 short names spells one folder two ways.
/// Letter case and the verbatim `\\?\` prefix differ from the stored target on every volume; the
/// short spelling also differs wherever the volume generates 8.3 names.
#[test]
fn every_spelling_of_the_target_folder_is_the_same_working_directory() -> TestResult {
    worker::run(|_, _| {
        let temporary = tempfile::tempdir()?;
        let folder = temporary.path().join("Generation with a long name");
        let folder = Directory::open(&folder, true)?.resolved()?.path().to_owned();
        let executable = folder.join("sugarsubstitute.exe");
        std::fs::write(&executable, b"inert, never executed")?;
        let spellings = [
            PathBuf::from(folder.to_string_lossy().to_uppercase()),
            PathBuf::from(format!(r"\\?\{}", folder.display())),
            crate::durable_storage::files::alias_fixture::short_spelling(&folder)?,
        ];
        let path = temporary.path().join("SugarSubstitute.lnk");
        for spelling in spellings {
            std::fs::write(&path, encode(&executable)?)?;
            set_working_directory(&path, &spelling)?;
            assert_eq!(target(&path)?, executable, "working directory {}", spelling.display());
        }
        Directory::open(&temporary.path().join("other"), true)?;
        set_working_directory(&path, &temporary.path().join("other"))?;
        assert!(target(&path).is_err(), "another folder is a different working directory");
        Ok(())
    })
}

fn set_working_directory(link_path: &Path, directory: &Path) -> io::Result<()> {
    let _apartment = Apartment::enter()?;
    // SAFETY: The fixture's local COM interfaces and strings outlive synchronous calls and
    // retire on this same worker before its apartment guard.
    unsafe {
        let link: IShellLinkW = CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER)?;
        let storage: IPersistFile = link.cast()?;
        storage.Load(&HSTRING::from(link_path.as_os_str()), STGM_READ)?;
        link.SetWorkingDirectory(&HSTRING::from(directory.as_os_str()))?;
        storage.Save(&HSTRING::from(link_path.as_os_str()), true)?;
    }
    Ok(())
}

#[test]
fn shell_serialization_creates_no_files_and_round_trips_unicode_targets() -> TestResult {
    worker::run(|_, _| {
        let temporary = tempfile::tempdir()?;
        let executable = temporary.path().join("第二 糖.exe");
        let bytes = encode(&executable)?;
        assert!(std::fs::read_dir(temporary.path())?.next().is_none());
        assert!(bytes.len() <= 65536);
        let path = temporary.path().join("SugarSubstitute.lnk");
        std::fs::write(&path, &bytes)?;
        assert!(super::super::launches(&target(&path)?, &executable)?);
        assert_eq!(std::fs::read(path)?, bytes);
        Ok(())
    })
}

#[test]
fn expanded_icon_metadata_preserves_the_same_launch_contract() -> TestResult {
    worker::run(|_, _| {
        let temporary = tempfile::tempdir()?;
        let executable = temporary.path().join("sugarsubstitute.exe");
        std::fs::write(&executable, b"inert, never executed")?;
        let path = temporary.path().join("icon.lnk");
        std::fs::write(&path, encode(&executable)?)?;
        let _apartment = Apartment::enter()?;
        // SAFETY: The fixture retains its local shell interfaces and input strings on this worker.
        unsafe {
            let link: IShellLinkW = CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER)?;
            let storage: IPersistFile = link.cast()?;
            storage.Load(&HSTRING::from(path.as_os_str()), STGM_READ)?;
            let windows = std::path::PathBuf::from(
                std::env::var_os("SystemRoot").ok_or("Windows system root absent")?,
            );
            link.SetIconLocation(
                &HSTRING::from(windows.join("System32/shell32.dll").as_os_str()),
                0,
            )?;
            storage.Save(&HSTRING::from(path.as_os_str()), true)?;
            storage.Load(&HSTRING::from(path.as_os_str()), STGM_READ)?;
            let data: IShellLinkDataList = link.cast()?;
            assert_ne!(data.GetFlags()? & SLDF_HAS_EXP_ICON_SZ.0 as u32, 0);
        }
        let before = std::fs::read(&path)?;
        assert!(super::super::launches(&target(&path)?, &executable)?);
        assert_eq!(std::fs::read(path)?, before);
        Ok(())
    })
}
