// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Recognize supported configuration capabilities without equating text with semantics.
use crate::{capability_source, error::Result};
use serde::Deserialize;
use std::collections::BTreeSet;
pub(super) const EXTENSIONS: &[&str] = &[
    "rs", "ts", "tsx", "mts", "cts", "js", "jsx", "mjs", "cjs", "c", "cc", "cpp", "cxx", "h",
    "hpp", "hxx", "m", "mm", "html", "py", "ps1",
];
/// Independent review categories prevent approval of one source from granting another route.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Capability {
    Environment,
    Arguments,
    Document,
    Codec,
    Backend,
    External,
}

/// Known persistent preference/configuration libraries require an explicit owner migration.
pub(super) fn backend(name: &str) -> bool {
    matches!(
        name,
        "config"
            | "figment"
            | "confy"
            | "preferences"
            | "directories"
            | "directories_next"
            | "winreg"
            | "registry"
            | "electron_store"
            | "electron_settings"
            | "keyv"
    )
}
fn native_environment(name: &str) -> bool {
    matches!(
        name,
        "getenv"
            | "getenv_s"
            | "_wgetenv"
            | "_wgetenv_s"
            | "GetEnvironmentVariableA"
            | "GetEnvironmentVariableW"
            | "GetEnvironmentStringsA"
            | "GetEnvironmentStringsW"
            | "SetEnvironmentVariableA"
            | "SetEnvironmentVariableW"
            | "setenv"
            | "putenv"
    )
}
fn native_backend(name: &str) -> bool {
    matches!(
        name,
        "localStorage"
            | "sessionStorage"
            | "QSettings"
            | "NSUserDefaults"
            | "RegOpenKeyExW"
            | "RegOpenKeyExA"
            | "RegCreateKeyExW"
            | "RegCreateKeyExA"
            | "RegSetValueExW"
            | "RegSetValueExA"
            | "RegQueryValueExW"
            | "RegQueryValueExA"
            | "CFPreferencesCopyAppValue"
            | "CFPreferencesSetAppValue"
    )
}
fn environment(path: &[String]) -> bool {
    let parts: Vec<_> = path.iter().map(String::as_str).collect();
    matches!(
        parts.as_slice(),
        ["std", "*"]
            | [
                "std",
                "env",
                "var" | "var_os" | "vars" | "vars_os" | "set_var" | "remove_var" | "*"
            ]
    ) || path.last().is_some_and(|name| native_environment(name))
}
fn arguments(path: &[String]) -> bool {
    matches!(
        path.iter().map(String::as_str).collect::<Vec<_>>().as_slice(),
        ["std", "*"] | ["std", "env", "args" | "args_os" | "*"]
    )
}
fn document(path: &[String]) -> bool {
    path.first().is_some_and(|name| name == "toml")
}
fn competing(path: &[String]) -> bool {
    path.first().is_some_and(|name| source_backend(name))
        || path.first().is_some_and(|name| name == "config")
            && path.iter().any(|name| name == "Config")
        || path.last().is_some_and(|name| native_backend(name))
}
/// Cargo checks every actual library identity; avoid treating the application's
/// process registry and ordinary configuration variables as persistent backends.
fn source_backend(name: &str) -> bool {
    matches!(name, "figment" | "confy" | "winreg" | "electron_store" | "electron_settings" | "keyv")
}
/// Inspect parsed Rust imports/reexports, qualified paths, wrappers and declarative macro
/// fragments even under inactive feature/platform/test attributes. Literals identify
/// existing host document routes, requiring review even when used through generic I/O.
pub(super) fn findings(source: &str, rust: bool) -> Result<BTreeSet<Capability>> {
    let mut found = BTreeSet::new();
    if rust {
        for (capability, path, fragments) in [
            (
                Capability::Environment,
                environment as fn(&[String]) -> bool,
                environment_fragments as fn(&BTreeSet<String>) -> bool,
            ),
            (Capability::Arguments, arguments, argument_fragments),
            (Capability::Codec, document, |names| names.contains("toml")),
            (Capability::Backend, competing, |names| {
                names.iter().any(|name| source_backend(name) || native_backend(name))
                    || names.contains("config") && names.contains("Config")
            }),
        ] {
            if !capability_source::findings(source, path, fragments, backend)?.is_empty() {
                found.insert(capability);
            }
        }
    } else {
        let names: BTreeSet<_> = source
            .split(|character: char| {
                !character.is_alphanumeric() && character != '_' && character != '$'
            })
            .collect();
        if names.iter().any(|name| native_environment(name))
            || names.contains("process") && names.contains("env")
            || names.contains("environ")
            || names.contains("GetEnvironmentVariable")
            || source.contains("$env:")
        {
            found.insert(Capability::Environment);
        }
        if names.contains("argv") || names.contains("CommandLineToArgvW") {
            found.insert(Capability::Arguments);
        }
        if names.iter().any(|name| native_backend(name))
            || ["electron-store", "electron-settings", "confy", "figment"]
                .iter()
                .any(|name| source.contains(name))
        {
            found.insert(Capability::Backend);
        }
        if names.iter().any(|name| {
            matches!(
                *name,
                "getMirrorConfig"
                    | "configureModelRoot"
                    | "ModelRootStore"
                    | "ModelRootConfiguration"
                    | "PYTHONDONTWRITEBYTECODE"
                    | "setPath"
                    | "SetUploadsEnabled"
            )
        }) {
            found.insert(Capability::External);
        }
    }
    if source.contains("startup.local.toml") || source.contains("sugarsubstitute-launch.local.json")
    {
        found.insert(Capability::Document);
    }
    Ok(found)
}
fn environment_fragments(names: &BTreeSet<String>) -> bool {
    names.iter().any(|name| native_environment(name))
        || names.contains("env")
            && ["var", "var_os", "vars", "vars_os", "set_var", "remove_var"]
                .iter()
                .any(|name| names.contains(*name))
}
fn argument_fragments(names: &BTreeSet<String>) -> bool {
    names.contains("env") && ["args", "args_os"].iter().any(|name| names.contains(*name))
}
