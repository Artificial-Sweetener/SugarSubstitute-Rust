// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Keep shell-link COM ownership on the admitted storage worker until every interface retires.
use std::{
    ffi::OsString,
    io,
    os::windows::ffi::OsStringExt,
    path::{Path, PathBuf},
};
use windows::{
    Win32::{
        Foundation::RPC_E_CHANGED_MODE,
        System::Com::{
            CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx,
            CoUninitialize, IPersistFile, IPersistStream, STATFLAG_NONAME, STATSTG, STGM_READ,
            STREAM_SEEK_SET,
        },
        UI::{
            Shell::{
                IShellLinkDataList, IShellLinkW, SHCreateMemStream, SLDF_ENABLE_TARGET_METADATA,
                SLDF_HAS_EXP_ICON_SZ, SLDF_HAS_ICONLOCATION, SLDF_HAS_ID_LIST, SLDF_HAS_LINK_INFO,
                SLDF_HAS_NAME, SLDF_HAS_RELPATH, SLDF_HAS_WORKINGDIR, SLDF_UNICODE, SLGP_RAWPATH,
                ShellLink,
            },
            WindowsAndMessaging::SW_SHOWNORMAL,
        },
    },
    core::{HSTRING, Interface},
};

#[cfg(test)]
mod tests;

struct Apartment(bool);
impl Apartment {
    fn enter() -> io::Result<Self> {
        // SAFETY: Balanced on this same worker; an existing apartment remains with its owner.
        let result = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) };
        if result == RPC_E_CHANGED_MODE {
            Ok(Self(false))
        } else {
            result.ok().map_err(io::Error::other)?;
            Ok(Self(true))
        }
    }
}
impl Drop for Apartment {
    fn drop(&mut self) {
        if self.0 {
            // SAFETY: Every local shell interface was dropped before this balanced release.
            unsafe { CoUninitialize() };
        }
    }
}

/// Serialize the complete shell link in memory on this worker without creating temporary files.
/// Only bounded shell bytes leave the apartment; no COM interface crosses a thread boundary.
pub(super) fn encode(executable: &Path) -> io::Result<Vec<u8>> {
    super::files::lexical(executable)?;
    let directory =
        executable.parent().ok_or_else(|| super::invalid("Shell target parent absent"))?;
    let _apartment = Apartment::enter()?;
    // SAFETY: COM is initialized on this thread. All interfaces and UTF-16 strings
    // stay live for their synchronous calls and retire before the apartment guard.
    unsafe {
        let link: IShellLinkW =
            CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER).map_err(io::Error::other)?;
        link.SetPath(&HSTRING::from(executable.as_os_str())).map_err(io::Error::other)?;
        link.SetWorkingDirectory(&HSTRING::from(directory.as_os_str()))
            .map_err(io::Error::other)?;
        link.SetIconLocation(&HSTRING::from(executable.as_os_str()), 0)
            .map_err(io::Error::other)?;
        link.SetDescription(&HSTRING::from("SugarSubstitute")).map_err(io::Error::other)?;
        link.SetArguments(&HSTRING::new()).map_err(io::Error::other)?;
        link.SetShowCmd(SW_SHOWNORMAL).map_err(io::Error::other)?;
        let storage: IPersistStream = link.cast().map_err(io::Error::other)?;
        if storage.GetSizeMax().map_err(io::Error::other)? > 65536 {
            return Err(super::invalid("Shell entry exceeds its serialization bound"));
        }
        let stream = SHCreateMemStream(None)
            .ok_or_else(|| io::Error::other("Shell stream allocation failed"))?;
        storage.Save(&stream, true).map_err(io::Error::other)?;
        let mut stat = STATSTG::default();
        stream.Stat(&mut stat, STATFLAG_NONAME).map_err(io::Error::other)?;
        if stat.cbSize == 0 || stat.cbSize > 65536 {
            return Err(super::invalid("Shell entry exceeds its serialization bound"));
        }
        stream.Seek(0, STREAM_SEEK_SET, None).map_err(io::Error::other)?;
        let mut bytes = vec![0; stat.cbSize as usize];
        let mut read = 0;
        stream
            .Read(bytes.as_mut_ptr().cast(), bytes.len() as u32, Some(&mut read))
            .ok()
            .map_err(io::Error::other)?;
        if read as usize != bytes.len() {
            return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "Shell stream ended early"));
        }
        Ok(bytes)
    }
}

/// Validate launch behavior while allowing Windows to encode display-only icon environment metadata.
pub(in crate::setup_storage) fn target(source: &Path) -> io::Result<PathBuf> {
    let _apartment = Apartment::enter()?;
    let mut path = [0_u16; 4097];
    let mut directory = [0_u16; 4097];
    let mut arguments = [0_u16; 4097];
    // SAFETY: The bounded native buffers and filename remain live through synchronous reads.
    // GetPath reads the stored target; Resolve is deliberately never called.
    unsafe {
        let link: IShellLinkW =
            CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER).map_err(io::Error::other)?;
        let storage: IPersistFile = link.cast().map_err(io::Error::other)?;
        storage.Load(&HSTRING::from(source.as_os_str()), STGM_READ).map_err(io::Error::other)?;
        link.GetPath(&mut path, std::ptr::null_mut(), SLGP_RAWPATH.0 as u32)
            .map_err(io::Error::other)?;
        link.GetWorkingDirectory(&mut directory).map_err(io::Error::other)?;
        link.GetArguments(&mut arguments).map_err(io::Error::other)?;
        let data: IShellLinkDataList = link.cast().map_err(io::Error::other)?;
        let flags = data.GetFlags().map_err(io::Error::other)?;
        let allowed = (SLDF_HAS_ID_LIST.0
            | SLDF_HAS_LINK_INFO.0
            | SLDF_HAS_NAME.0
            | SLDF_HAS_RELPATH.0
            | SLDF_HAS_WORKINGDIR.0
            | SLDF_HAS_ICONLOCATION.0
            | SLDF_HAS_EXP_ICON_SZ.0
            | SLDF_UNICODE.0
            | SLDF_ENABLE_TARGET_METADATA.0) as u32;
        if arguments[0] != 0
            || flags & !allowed != 0
            || link.GetShowCmd().map_err(io::Error::other)? != SW_SHOWNORMAL
        {
            return Err(super::invalid(
                "The existing shell entry has custom launch behavior; it has been preserved",
            ));
        }
    }
    let target = bounded_path(&path)?;
    if target.parent() != Some(bounded_path(&directory)?.as_path()) {
        return Err(super::invalid(
            "The existing shell entry has a different working directory; it has been preserved",
        ));
    }
    Ok(target)
}

/// Reject truncated native values instead of comparing a clipped target or working directory.
fn bounded_path(path: &[u16]) -> io::Result<PathBuf> {
    let end = path
        .iter()
        .position(|unit| *unit == 0)
        .filter(|end| *end > 0 && *end < 4096)
        .ok_or_else(|| super::invalid("Shell target exceeds its bound"))?;
    Ok(PathBuf::from(OsString::from_wide(&path[..end])))
}
