// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Recognize native filesystem and dynamic-entry-point identities.
/// Shell file dialog configuration methods. They describe what the user may choose and never
/// touch a file, although several share the `SetFile` prefix of filesystem functions.
const SHELL_DIALOG_METHODS: [&str; 9] = [
    "SetFileName",
    "SetFileTypes",
    "SetFileTypeIndex",
    "SetFolder",
    "SetDefaultFolder",
    "SetDefaultExtension",
    "SetFileNameLabel",
    "SetOkButtonLabel",
    "SetTitle",
];

/// Native file acquisition, inspection, mutation, dynamic native loading and publication identities.
pub(super) fn known(name: &str) -> bool {
    if SHELL_DIALOG_METHODS.contains(&name) {
        return false;
    }
    [
        "CreateFile",
        "GetFile",
        "SetFile",
        "MoveFile",
        "DeleteFile",
        "ReplaceFile",
        "CreateDirectory",
        "RemoveDirectory",
        "FindFirstFile",
        "FindNextFile",
    ]
    .iter()
    .any(|prefix| name.starts_with(prefix))
        || matches!(
            name,
            "OpenFile"
                | "OpenFileById"
                | "IPersistFile"
                | "SHCreateStreamOnFileEx"
                | "StgOpenStorageEx"
                | "CreateFileA"
                | "CreateFileW"
                | "CreateFile2"
                | "NtCreateFile"
                | "NtOpenFile"
                | "ReadFile"
                | "WriteFile"
                | "ReadFileEx"
                | "WriteFileEx"
                | "GetFileInformationByHandle"
                | "GetFileInformationByHandleEx"
                | "SetFileInformationByHandle"
                | "GetFileAttributesA"
                | "GetFileAttributesW"
                | "GetFileAttributesExA"
                | "GetFileAttributesExW"
                | "MoveFileA"
                | "MoveFileW"
                | "MoveFileExA"
                | "MoveFileExW"
                | "MoveFileWithProgressA"
                | "MoveFileWithProgressW"
                | "ReplaceFileA"
                | "ReplaceFileW"
                | "DeleteFileA"
                | "DeleteFileW"
                | "CreateDirectoryA"
                | "CreateDirectoryW"
                | "RemoveDirectoryA"
                | "RemoveDirectoryW"
                | "FindFirstFileA"
                | "FindFirstFileW"
                | "FindFirstFileExA"
                | "FindFirstFileExW"
                | "FindNextFileA"
                | "FindNextFileW"
                | "NtSetInformationFile"
                | "NtQueryInformationFile"
                | "NtQueryDirectoryFile"
                | "GetFinalPathNameByHandleW"
                | "GetFullPathNameW"
                | "GetFileSizeEx"
                | "FlushFileBuffers"
                | "SetEndOfFile"
                | "CreateHardLinkW"
                | "CreateSymbolicLinkW"
                | "GetTempPathW"
                | "GetTempFileNameW"
                | "GetProcAddress"
                | "LoadLibraryA"
                | "LoadLibraryW"
                | "LoadLibraryExW"
                | "dlsym"
                | "dlopen"
                | "renameat"
                | "renameat2"
                | "openat"
                | "openat2"
                | "unlinkat"
                | "mkdirat"
                | "fopen"
                | "freopen"
                | "fopen_s"
                | "_wfopen"
                | "_wopen"
                | "_sopen_s"
        )
}

/// POSIX foreign symbols are restricted even when the authored Rust function is renamed.
pub(super) fn posix(name: &str) -> bool {
    matches!(
        name,
        "open"
            | "creat"
            | "close"
            | "read"
            | "write"
            | "stat"
            | "lstat"
            | "fstat"
            | "mmap"
            | "munmap"
            | "chmod"
            | "chown"
            | "ftruncate"
            | "truncate"
            | "link"
            | "symlink"
            | "unlink"
            | "rename"
            | "mkdir"
            | "rmdir"
            | "opendir"
            | "readdir"
            | "realpath"
            | "access"
    )
}

#[cfg(test)]
mod tests {
    use super::known;

    /// Filesystem functions sharing the dialog methods' prefix stay restricted.
    #[test]
    fn set_file_filesystem_functions_remain_restricted() {
        for name in [
            "SetFileInformationByHandle",
            "SetFilePointerEx",
            "SetFileAttributesW",
            "SetFileTime",
            "SetFileValidData",
            "SetFileShortNameW",
        ] {
            assert!(known(name), "{name}");
        }
    }

    /// Exactly the shell dialog configuration methods are accepted.
    #[test]
    fn shell_dialog_configuration_methods_are_not_filesystem_access() {
        for name in [
            "SetFileName",
            "SetFileTypes",
            "SetFileTypeIndex",
            "SetFolder",
            "SetDefaultFolder",
            "SetDefaultExtension",
            "SetFileNameLabel",
            "SetOkButtonLabel",
            "SetTitle",
        ] {
            assert!(!known(name), "{name}");
        }
        assert!(known("SetFileNames"), "only the exact method names are accepted");
    }
}
