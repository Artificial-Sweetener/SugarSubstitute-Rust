// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Preserve operational cwd semantics independently of canonical launch identity paths.
use std::error::Error;
use sugarsubstitute_supervision::WindowsLaunchCommand;

fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().collect()
}

fn directory(source: &[u16]) -> Result<Vec<u16>, Box<dyn Error>> {
    let command =
        WindowsLaunchCommand::from_utf16(&wide(r"\\?\E:\ComfyUI\python.exe"), source, &[])?;
    let actual = command.working_directory_utf16();
    Ok(actual[..actual.len() - 1].to_vec())
}

/// Comfy's root-relative model resolution needs a drive root instead of a namespace root.
#[test]
fn canonical_comfy_cwd_uses_ordinary_spelling_without_changing_executable_or_argv()
-> Result<(), Box<dyn Error>> {
    let executable = wide(r"\\?\E:\ComfyUI\venv\Scripts\python.exe");
    let cwd = wide(r"\\?\E:\ComfyUI");
    let arguments = [wide(r"E:\ComfyUI\main.py"), wide("--port"), wide("18421")];
    let command = WindowsLaunchCommand::from_utf16(&executable, &cwd, &arguments)?;
    assert_eq!(command.working_directory_utf16(), wide("E:\\ComfyUI\0"));
    assert_eq!(command.application_name_utf16(), [executable.as_slice(), &[0]].concat());
    assert_eq!(
        command.command_line_utf16(),
        wide(
            "\"\\\\?\\E:\\ComfyUI\\venv\\Scripts\\python.exe\" \"E:\\ComfyUI\\main.py\" \"--port\" \"18421\"\0"
        )
    );
    assert_eq!(cwd, wide(r"\\?\E:\ComfyUI"));
    Ok(())
}

#[test]
fn short_drive_and_unc_directories_preserve_roots_unicode_and_trailing_separator()
-> Result<(), Box<dyn Error>> {
    for (source, expected) in [
        (r"\\?\E:\", r"E:\"),
        (r"\\?\E:\東京 workspace\", r"E:\東京 workspace\"),
        (r"\\?\UNC\server\share", r"\\server\share"),
        (r"\\?\UNC\server\share\subfolder\", r"\\server\share\subfolder\"),
        (r"E:\ordinary\", r"E:\ordinary\"),
        ("E:/ordinary", "E:/ordinary"),
    ] {
        assert_eq!(directory(&wide(source))?, wide(expected), "{source}");
    }
    let mut source = wide(r"\\?\E:\");
    source.push(0xd800);
    assert_eq!(directory(&source)?, source[4..]);
    Ok(())
}

#[test]
fn ordinary_spelling_stops_at_the_win32_length_boundary() -> Result<(), Box<dyn Error>> {
    for (prefix, removed) in [(r"\\?\E:\", 4), (r"\\?\UNC\server\share\", 6)] {
        for length in [259, 260, 261, 1000] {
            let mut source = wide(prefix);
            source.extend(std::iter::repeat_n(b'x' as u16, length + removed - source.len()));
            let actual = directory(&source)?;
            if length < 260 {
                assert_eq!(actual.len(), length);
                assert!(!actual.starts_with(&wide(r"\\?\")));
            } else {
                assert_eq!(actual, source);
            }
        }
    }
    Ok(())
}

#[test]
fn extended_only_component_meaning_is_never_changed() -> Result<(), Box<dyn Error>> {
    for component in [
        "trailing.",
        "trailing ",
        "CON",
        "con.txt",
        "PRN",
        "AUX",
        "NUL",
        "NUL .txt",
        "CLOCK$",
        "CONIN$",
        "CONOUT$",
        "COM1",
        "lpt9.ext",
        "COM¹",
        "LPT²",
        "LPT³",
    ] {
        for root in [r"\\?\E:\", r"\\?\UNC\server\share\"] {
            let source = wide(&format!("{root}{component}\\child"));
            assert_eq!(directory(&source)?, source, "{component}");
        }
    }
    for source in [r"\\?\E:\folder\\child", r"\\?\E:\folder\\"] {
        assert_eq!(directory(&wide(source))?, wide(source));
    }
    for component in ["context", "com0", "LPT10", "console.txt", "CONSOLE"] {
        let source = wide(&format!(r"\\?\E:\{component}"));
        assert_eq!(directory(&source)?, source[4..]);
    }
    Ok(())
}
