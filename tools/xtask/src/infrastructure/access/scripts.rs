// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Classify decoded script, native, C# and build-language filesystem access.
use crate::source_tokens;
use std::collections::BTreeSet;
/// Conservative decoded tokens include quoted property names and escaped script identifiers.
pub(in super::super) fn classify(source: &str, extension: &str) -> Vec<String> {
    let source =
        if extension.eq_ignore_ascii_case("ps1") { source.replace('`', "") } else { source.into() };
    let extension = extension.to_ascii_lowercase();
    let script = matches!(
        extension.as_str(),
        "js" | "jsx" | "mjs" | "cjs" | "ts" | "tsx" | "mts" | "cts" | "html" | "patch"
    );
    let python = extension == "py";
    let dotnet = matches!(extension.as_str(), "cs" | "ps1");
    let native = matches!(
        extension.as_str(),
        "c" | "cc" | "cpp" | "cxx" | "h" | "hpp" | "hxx" | "m" | "mm" | "patch"
    );
    let mut found = BTreeSet::new();
    for token in source_tokens::tokens(&source) {
        if super::native::known(&token)
            || script
                && matches!(
                    token.as_str(),
                    "fs" | "node:fs"
                        | "node:fs/promises"
                        | "fs/promises"
                        | "fs-extra"
                        | "original-fs"
                        | "getBuiltinModule"
                        | "_linkedBinding"
                        | "internalBinding"
                        | "readFile"
                        | "readFileSync"
                        | "writeFile"
                        | "writeFileSync"
                        | "mkdirSync"
                        | "mkdir"
                        | "mkdtemp"
                        | "rename"
                        | "renameSync"
                        | "unlink"
                        | "rmSync"
                        | "createReadStream"
                        | "createWriteStream"
                        | "existsSync"
                        | "statSync"
                        | "lstatSync"
                        | "accessSync"
                        | "copyFile"
                        | "copyFileSync"
                        | "opendir"
                )
            || python
                && matches!(
                    token.as_str(),
                    "pathlib"
                        | "shutil"
                        | "open"
                        | "tempfile"
                        | "TemporaryDirectory"
                        | "NamedTemporaryFile"
                        | "TemporaryFile"
                        | "mkstemp"
                        | "read_bytes"
                        | "read_text"
                        | "write_bytes"
                        | "write_text"
                        | "scandir"
                        | "iterdir"
                        | "listdir"
                        | "makedirs"
                        | "rmdir"
                        | "remove"
                        | "replace"
                        | "stat"
                        | "lstat"
                        | "readlink"
                        | "symlink"
                        | "glob"
                        | "rglob"
                        | "getcwd"
                        | "chdir"
                        | "rename"
                        | "unlink"
                )
            || dotnet && dotnet_access(&token, extension == "ps1")
            || native
                && matches!(token.as_str(), "fstream" | "ifstream" | "ofstream" | "filesystem")
        {
            found.insert(token);
        }
    }
    if extension.eq_ignore_ascii_case("ps1") {
        let lower = source.to_ascii_lowercase();
        for command in [
            "get-content",
            "set-content",
            "add-content",
            "remove-item",
            "copy-item",
            "move-item",
            "new-item",
            "get-item",
            "get-childitem",
            "test-path",
            "out-file",
            "get-acl",
            "set-acl",
            "resolve-path",
        ] {
            if lower.contains(command) {
                found.insert(command.into());
            }
        }
    }
    if matches!(extension.as_str(), "gn" | "gni" | "cmake") {
        for token in source_tokens::tokens(&source) {
            if matches!(
                token.to_ascii_lowercase().as_str(),
                "read_file" | "write_file" | "exec_script" | "copy" | "file" | "configure_file"
            ) {
                found.insert(token);
            }
        }
    }
    found.into_iter().collect()
}

/// PowerShell binds .NET types and methods without case sensitivity; C# preserves their case.
fn dotnet_access(token: &str, powershell: bool) -> bool {
    [
        "FileStream",
        "FileInfo",
        "DirectoryInfo",
        "File",
        "Directory",
        "ReadAllBytes",
        "WriteAllBytes",
        "ReadAllText",
        "WriteAllText",
        "GetTempFileName",
    ]
    .iter()
    .any(|name| token == *name || powershell && token.eq_ignore_ascii_case(name))
}
