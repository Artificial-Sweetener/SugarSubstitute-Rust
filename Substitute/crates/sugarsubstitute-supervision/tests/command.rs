// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Check portable native-input preparation against independent Microsoft CRT parsing.
use std::{collections::TryReserveError, error::Error};
use sugarsubstitute_supervision::{
    LaunchCommandError, MAX_WINDOWS_ARGUMENTS, WindowsLaunchCommand,
};
type TestResult = Result<(), Box<dyn Error>>;
fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().collect()
}
fn prepare(arguments: &[Vec<u16>]) -> Result<WindowsLaunchCommand, LaunchCommandError> {
    WindowsLaunchCommand::from_utf16(&wide(r"E:\x.exe"), &wide(r"E:\"), arguments)
}

// Independent reader: argv0 has pathname quoting; later tokens use CRT slash parity
// and doubled quotes. It reads general command text rather than undoing encoder steps.
fn parse(command: &[u16]) -> Vec<Vec<u16>> {
    let command = &command[..command.len() - 1];
    let mut result = Vec::new();
    let mut cursor = 0;
    let mut program = Vec::new();
    let mut quoted = false;
    while cursor < command.len() {
        let unit = command[cursor];
        if unit == 34 {
            quoted = !quoted;
        } else if !quoted && matches!(unit, 9 | 32) {
            break;
        } else {
            program.push(unit);
        }
        cursor += 1;
    }
    result.push(program);
    while cursor < command.len() {
        while cursor < command.len() && matches!(command[cursor], 9 | 32) {
            cursor += 1;
        }
        if cursor == command.len() {
            break;
        }
        let mut argument = Vec::new();
        let mut quoted = false;
        while cursor < command.len() {
            if !quoted && matches!(command[cursor], 9 | 32) {
                break;
            }
            let mut slashes = 0;
            while cursor < command.len() && command[cursor] == 92 {
                slashes += 1;
                cursor += 1;
            }
            if cursor < command.len() && command[cursor] == 34 {
                argument.extend(std::iter::repeat_n(92, slashes / 2));
                if slashes % 2 != 0 {
                    argument.push(34);
                } else if quoted && command.get(cursor + 1) == Some(&34) {
                    argument.push(34);
                    cursor += 1;
                } else {
                    quoted = !quoted;
                }
                cursor += 1;
            } else {
                argument.extend(std::iter::repeat_n(92, slashes));
                if cursor == command.len() || !quoted && matches!(command[cursor], 9 | 32) {
                    break;
                }
                argument.push(command[cursor]);
                cursor += 1;
            }
        }
        result.push(argument);
    }
    result
}

#[test]
fn literal_crt_examples_include_empty_quotes_slash_parity_and_trailing_slashes() -> TestResult {
    for (argument, expected) in [
        ("", r#""""#),
        ("a b", r#""a b""#),
        ("a\tb", "\"a\tb\""),
        (r#"a"b"#, r#""a\"b""#),
        (r#"a\"b"#, r#""a\\\"b""#),
        (r"a\", r#""a\\""#),
        (r"a\\", r#""a\\\\""#),
    ] {
        let input = wide(argument);
        let command = prepare(std::slice::from_ref(&input))?;
        let expected_line = format!("\"E:\\x.exe\" {expected}\0");
        assert_eq!(command.command_line_utf16(), wide(&expected_line));
        assert_eq!(parse(command.command_line_utf16()), vec![wide(r"E:\x.exe"), input]);
    }
    Ok(())
}

#[test]
fn independent_reader_preserves_all_short_quote_whitespace_and_backslash_combinations() -> TestResult
{
    let alphabet = [97, 32, 9, 92, 34];
    for length in 0..=4_u32 {
        for number in 0..5_usize.pow(length) {
            let mut value = number;
            let mut input = Vec::new();
            for _ in 0..length {
                input.push(alphabet[value % 5]);
                value /= 5;
            }
            let command = prepare(std::slice::from_ref(&input))?;
            assert_eq!(parse(command.command_line_utf16()), vec![wide(r"E:\x.exe"), input]);
        }
    }
    Ok(())
}

#[test]
fn unicode_shell_characters_and_unpaired_surrogates_remain_exact_code_units() -> TestResult {
    let arguments =
        vec![wide("東京😀e\u{301}"), wide("%PATH% & | < > ^\n"), vec![0xd800, 0xdc00, 0xdfff]];
    let command = prepare(&arguments)?;
    let mut expected = vec![wide(r"E:\x.exe")];
    expected.extend(arguments);
    assert_eq!(parse(command.command_line_utf16()), expected);
    Ok(())
}

#[test]
fn executable_argv0_uses_distinct_path_quoting_and_explicit_application_name() -> TestResult {
    let executable = wide(r"E:\東京 Program Files\app.exe");
    let cwd = wide(r"E:\separate cwd\");
    let command = WindowsLaunchCommand::from_utf16(&executable, &cwd, &[])?;
    assert_eq!(parse(command.command_line_utf16()), vec![executable.clone()]);
    let mut application = executable;
    application.push(0);
    let mut directory = cwd;
    directory.push(0);
    assert_eq!(command.application_name_utf16(), application);
    assert_eq!(command.working_directory_utf16(), directory);
    assert_eq!(command.command_line_utf16().last(), Some(&0));
    let mut buffers = command.into_native_parts();
    assert_eq!(buffers.application_name, application);
    assert_eq!(buffers.working_directory, directory);
    buffers.command_line[0] = 32;
    assert_eq!(buffers.application_name, application);
    Ok(())
}

#[test]
fn supported_fully_qualified_drive_unc_and_extended_paths_are_preserved() -> TestResult {
    for (executable, directory) in [
        (r"E:\app.exe", r"E:\"),
        ("E:/app.exe", "E:/work"),
        (r"\\server\share\app.exe", r"\\server\share"),
        (r"\\?\E:\long root\app.exe", r"\\?\E:\long root"),
        (r"\\?\UNC\server\share\app.exe", r"\\?\UNC\server\share"),
    ] {
        let executable = wide(executable);
        let command = WindowsLaunchCommand::from_utf16(&executable, &wide(directory), &[])?;
        assert_eq!(parse(command.command_line_utf16()), vec![executable]);
    }
    Ok(())
}

#[test]
fn relative_device_dot_root_and_quoted_executable_paths_are_rejected() {
    for executable in [
        "",
        "app.exe",
        r"E:app.exe",
        r"\app.exe",
        r"E:\",
        r"E:\folder\",
        r"\\server\share",
        r"\\\server\share\app.exe",
        r"\\server\\app.exe",
        r"\\.\pipe\thing",
        r"\\?\GLOBALROOT\app.exe",
        r"E:\..\app.exe",
        r"E:\.\app.exe",
        r#"E:\a"b.exe"#,
        r"\\?\E:/app.exe",
        r"E:\app.exe:stream",
    ] {
        assert!(matches!(
            WindowsLaunchCommand::from_utf16(&wide(executable), &wide(r"E:\"), &[]),
            Err(LaunchCommandError::InvalidExecutable)
        ));
    }
}

#[test]
fn invalid_working_directory_is_rejected_independently_of_executable() {
    for directory in [
        "",
        "work",
        "E:work",
        r"\work",
        r"E:\..",
        r"\\server",
        r"\\.\pipe\thing",
        r#"E:\quoted"path"#,
    ] {
        assert!(matches!(
            WindowsLaunchCommand::from_utf16(&wide(r"E:\app.exe"), &wide(directory), &[]),
            Err(LaunchCommandError::InvalidWorkingDirectory)
        ));
    }
}

#[test]
fn interior_nul_in_every_native_input_is_rejected() {
    assert!(matches!(
        WindowsLaunchCommand::from_utf16(&wide("E:\\app\0.exe"), &wide(r"E:\"), &[]),
        Err(LaunchCommandError::InteriorNul)
    ));
    assert!(matches!(
        WindowsLaunchCommand::from_utf16(&wide(r"E:\app.exe"), &wide("E:\\work\0"), &[]),
        Err(LaunchCommandError::InteriorNul)
    ));
    assert!(matches!(prepare(&[wide("a\0b")]), Err(LaunchCommandError::InteriorNul)));
}

#[test]
fn source_count_and_size_fail_before_content_validation() -> TestResult {
    assert_eq!(MAX_WINDOWS_ARGUMENTS, 64);
    assert!(prepare(&vec![Vec::new(); 64]).is_ok());
    assert!(matches!(prepare(&vec![vec![0]; 65]), Err(LaunchCommandError::TooManyArguments)));
    assert!(matches!(prepare(&[vec![0; 32767]]), Err(LaunchCommandError::SourceTooLong)));
    assert!(matches!(
        prepare(&[vec![97; 20000], vec![97; 20000]]),
        Err(LaunchCommandError::SourceTooLong)
    ));
    assert!(matches!(
        WindowsLaunchCommand::from_utf16(&vec![0; 32767], &[], &[]),
        Err(LaunchCommandError::SourceTooLong)
    ));
    let mut directory = wide(r"E:\");
    directory.resize(32766, 97);
    assert_eq!(
        WindowsLaunchCommand::from_utf16(&wide(r"E:\x.exe"), &directory, &[])?
            .working_directory_utf16()
            .len(),
        32767
    );
    directory.push(97);
    assert!(matches!(
        WindowsLaunchCommand::from_utf16(&wide(r"E:\x.exe"), &directory, &[]),
        Err(LaunchCommandError::SourceTooLong)
    ));
    Ok(())
}

#[test]
fn exact_native_length_includes_argv0_quotes_separators_and_final_nul() -> TestResult {
    let command = prepare(&[vec![97; 32753]])?;
    assert_eq!(command.command_line_utf16().len(), 32767);
    assert_eq!(command.command_line_utf16().last(), Some(&0));
    assert!(matches!(prepare(&[vec![97; 32754]]), Err(LaunchCommandError::CommandLineTooLong)));
    Ok(())
}

#[test]
fn quote_and_trailing_backslash_expansion_is_checked_before_output_allocation() -> TestResult {
    for expanded in [34, 92] {
        let mut exact = vec![expanded; 16376];
        exact.push(97);
        if expanded == 92 {
            exact.insert(0, 97);
            exact.pop();
        }
        let command = prepare(std::slice::from_ref(&exact))?;
        assert_eq!(command.command_line_utf16().len(), 32767);
        assert_eq!(parse(command.command_line_utf16()), vec![wide(r"E:\x.exe"), exact]);
        assert!(matches!(
            prepare(&[vec![expanded; 16377]]),
            Err(LaunchCommandError::CommandLineTooLong)
        ));
    }
    Ok(())
}

#[test]
fn debug_errors_and_transferred_buffers_redact_paths_and_argv_but_keep_allocation_cause()
-> TestResult {
    let command = prepare(&[wide("SECRET_ARG")])?;
    let debug = format!("{command:?}");
    assert!(!debug.contains("SECRET_ARG"));
    assert!(!debug.contains("x.exe"));
    assert!(!format!("{:?}", command.into_native_parts()).contains("SECRET_ARG"));
    let mut vector: Vec<u16> = Vec::new();
    let Err(cause) = vector.try_reserve(usize::MAX) else {
        return Err("expected reserve overflow".into());
    };
    let error = LaunchCommandError::Allocation(cause);
    assert!(
        error.source().is_some_and(|source| source.downcast_ref::<TryReserveError>().is_some())
    );
    assert_eq!(format!("{error:?}"), format!("{error}"));
    Ok(())
}

#[test]
fn native_os_conversion_is_exact_for_windows_utf16_without_filesystem_access() -> TestResult {
    let units = vec![0xd800, 34, 92, 0xdfff];
    let command = prepare(std::slice::from_ref(&units))?;
    assert_eq!(parse(command.command_line_utf16()), vec![wide(r"E:\x.exe"), units.clone()]);
    #[cfg(windows)]
    {
        use std::{ffi::OsString, os::windows::ffi::OsStringExt, path::Path};
        let native = WindowsLaunchCommand::from_os(
            Path::new(r"E:\x.exe"),
            Path::new(r"E:\"),
            &[OsString::from_wide(&units)],
        )?;
        assert_eq!(native.command_line_utf16(), command.command_line_utf16());
        let oversized = OsString::from_wide(&vec![34; 16377]);
        assert!(matches!(
            WindowsLaunchCommand::from_os(Path::new(r"E:\x.exe"), Path::new(r"E:\"), &[oversized]),
            Err(LaunchCommandError::CommandLineTooLong)
        ));
        let too_many = vec![OsString::from_wide(&[0]); 65];
        assert!(matches!(
            WindowsLaunchCommand::from_os(Path::new(r"E:\x.exe"), Path::new(r"E:\"), &too_many),
            Err(LaunchCommandError::TooManyArguments)
        ));
    }
    Ok(())
}
