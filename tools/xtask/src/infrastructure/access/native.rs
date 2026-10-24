// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Recognize native filesystem and dynamic-entry-point identities.
/// Native file acquisition, inspection, mutation, dynamic native loading and publication identities.
pub(super) fn known(name: &str) -> bool {
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
