// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Prepare explicit Windows executable/cwd and CRT-compatible argv without shell execution.
//! CreateProcessW requires a mutable command line of at most 32767 UTF-16 units
//! including NUL. Its application-name parameter is supplied separately. Microsoft
//! C startup treats argv0 as a quoted pathname, independently of later argv escapes.
//! See [CreateProcessW](https://learn.microsoft.com/en-us/windows/win32/api/processthreadsapi/nf-processthreadsapi-createprocessw)
//! and [Microsoft CRT argument parsing](https://learn.microsoft.com/en-us/cpp/c-language/parsing-c-command-line-arguments).
use crate::{LaunchCommandError, arguments};
use std::fmt;
mod paths;
const MAX_UNITS: usize = 32767;
/// Maximum number of arguments after argv0; matches supported launch configuration.
pub const MAX_WINDOWS_ARGUMENTS: usize = 64;

/// Transfer owned NUL-terminated buffers to the native CreateProcessW caller.
/// Buffers confer no canonical trust, containment, privilege or process ownership.
/// The caller keeps all three live through the call and permits CreateProcessW to
/// mutate only `command_line`. Changed buffers no longer retain validation guarantees.
pub struct WindowsLaunchBuffers {
    /// Explicit lpApplicationName, unquoted; never pass NULL or infer it from argv0.
    pub application_name: Vec<u16>,
    /// Explicit lpCurrentDirectory, independent of ambient process cwd.
    pub working_directory: Vec<u16>,
    /// Mutable lpCommandLine, including exact executable argv0 and terminating NUL.
    pub command_line: Vec<u16>,
}
impl fmt::Debug for WindowsLaunchBuffers {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.debug_struct("WindowsLaunchBuffers").finish_non_exhaustive()
    }
}
/// Immutable bounded Windows native inputs awaiting a separate trusted launch owner.
/// Construction validates lexical shape only and performs no I/O or process operation.
pub struct WindowsLaunchCommand {
    buffers: WindowsLaunchBuffers,
}
impl fmt::Debug for WindowsLaunchCommand {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.debug_struct("WindowsLaunchCommand").finish_non_exhaustive()
    }
}
impl WindowsLaunchCommand {
    /// Prepare exact Windows code units; inputs exclude terminating NUL.
    ///
    /// Accept fully-qualified drive/UNC and extended drive/UNC lexical paths. Reject
    /// root-relative/drive-relative/device paths, dot components, executable roots,
    /// embedded path quotes and NUL. UTF-16 units, including unpaired surrogates,
    /// remain exact. Canonical filesystem identity and trust are not established.
    ///
    /// Source count/length/aggregate bounds are checked before content scans or
    /// allocations. Exact quoting expansion is checked before native buffer allocation.
    /// Errors redact paths/argv and retain original allocation causes.
    pub fn from_utf16(
        executable: &[u16],
        working_directory: &[u16],
        arguments: &[Vec<u16>],
    ) -> Result<Self, LaunchCommandError> {
        preflight(executable.len(), working_directory.len(), arguments.iter().map(Vec::len))?;
        if executable.contains(&0)
            || working_directory.contains(&0)
            || arguments.iter().any(|argument| argument.contains(&0))
        {
            return Err(LaunchCommandError::InteriorNul);
        }
        if !paths::valid(executable, true) {
            return Err(LaunchCommandError::InvalidExecutable);
        }
        if !paths::valid(working_directory, false) {
            return Err(LaunchCommandError::InvalidWorkingDirectory);
        }
        let length = command_line_length(
            executable.len(),
            arguments.iter().map(|argument| arguments::quoted_length(argument.iter().copied())),
        )?;
        let mut command_line = arguments::reserved(length)?;
        command_line.push(b'"' as u16);
        command_line.extend_from_slice(executable);
        command_line.push(b'"' as u16);
        for argument in arguments {
            command_line.push(b' ' as u16);
            arguments::append_quoted(&mut command_line, argument);
        }
        command_line.push(0);
        Ok(Self {
            buffers: WindowsLaunchBuffers {
                application_name: arguments::terminated(executable)?,
                working_directory: arguments::terminated(working_directory)?,
                command_line,
            },
        })
    }
    /// Borrow immutable, explicit, unquoted lpApplicationName including its final NUL.
    pub fn application_name_utf16(&self) -> &[u16] {
        &self.buffers.application_name
    }
    /// Borrow immutable lpCurrentDirectory including its final NUL.
    pub fn working_directory_utf16(&self) -> &[u16] {
        &self.buffers.working_directory
    }
    /// Borrow immutable quoted argv including argv0 and its final NUL.
    pub fn command_line_utf16(&self) -> &[u16] {
        &self.buffers.command_line
    }
    /// Consume validation ownership and transfer named buffers for one native launch.
    /// No raw handle/runtime or process authority is returned.
    pub fn into_native_parts(self) -> WindowsLaunchBuffers {
        self.buffers
    }

    /// Encode native Windows paths/arguments exactly without lossy Unicode conversion.
    ///
    /// Bounded encoding-count and exact quoting-size passes precede every temporary
    /// and native-buffer allocation. Shape checks and CRT quoting then use the same
    /// `from_utf16` owner; no filesystem resolution is performed.
    #[cfg(windows)]
    pub fn from_os(
        executable: &std::path::Path,
        working_directory: &std::path::Path,
        arguments: &[std::ffi::OsString],
    ) -> Result<Self, LaunchCommandError> {
        use std::os::windows::ffi::OsStrExt;
        if arguments.len() > MAX_WINDOWS_ARGUMENTS {
            return Err(LaunchCommandError::TooManyArguments);
        }
        let executable_length = executable.as_os_str().encode_wide().take(MAX_UNITS).count();
        let directory_length = working_directory.as_os_str().encode_wide().take(MAX_UNITS).count();
        let mut lengths = [0; MAX_WINDOWS_ARGUMENTS];
        for (length, argument) in lengths.iter_mut().zip(arguments) {
            *length = argument.encode_wide().take(MAX_UNITS).count();
        }
        preflight(executable_length, directory_length, lengths[..arguments.len()].iter().copied())?;
        command_line_length(
            executable_length,
            arguments.iter().map(|argument| arguments::quoted_length(argument.encode_wide())),
        )?;
        let encode = |source: &std::ffi::OsStr, length| {
            let mut buffer = arguments::reserved(length)?;
            buffer.extend(source.encode_wide());
            Ok::<_, LaunchCommandError>(buffer)
        };
        let executable = encode(executable.as_os_str(), executable_length)?;
        let directory = encode(working_directory.as_os_str(), directory_length)?;
        let mut encoded = Vec::new();
        encoded.try_reserve_exact(arguments.len()).map_err(LaunchCommandError::Allocation)?;
        for (argument, length) in arguments.iter().zip(lengths) {
            encoded.push(encode(argument, length)?);
        }
        Self::from_utf16(&executable, &directory, &encoded)
    }
}
fn command_line_length(
    executable: usize,
    arguments: impl Iterator<Item = usize>,
) -> Result<usize, LaunchCommandError> {
    let mut length = executable + 3;
    for argument in arguments {
        length += 1 + argument;
        if length > MAX_UNITS {
            return Err(LaunchCommandError::CommandLineTooLong);
        }
    }
    if length > MAX_UNITS {
        return Err(LaunchCommandError::CommandLineTooLong);
    }
    Ok(length)
}
fn preflight(
    executable: usize,
    working_directory: usize,
    arguments: impl ExactSizeIterator<Item = usize>,
) -> Result<(), LaunchCommandError> {
    if arguments.len() > MAX_WINDOWS_ARGUMENTS {
        return Err(LaunchCommandError::TooManyArguments);
    }
    if executable >= MAX_UNITS || working_directory >= MAX_UNITS {
        return Err(LaunchCommandError::SourceTooLong);
    }
    let mut total = executable;
    for length in arguments {
        if length >= MAX_UNITS {
            return Err(LaunchCommandError::SourceTooLong);
        }
        total += length;
        if total >= MAX_UNITS {
            return Err(LaunchCommandError::SourceTooLong);
        }
    }
    Ok(())
}
