// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Inspect public configuration contracts through the shared module, alias and macro resolver.
use crate::{error::Result, execution_exports, workspace::Dependency};
use std::path::Path;

/// Raw maps, environment iterators, files and backend modules cannot escape document validation.
fn restricted(path: &[String]) -> bool {
    let Some(last) = path.last().map(String::as_str) else { return false };
    raw_name(last)
        || path == ["std"]
        || path.first().is_some_and(|first| {
            matches!(first.as_str(), "toml" | "serde_json" | "config" | "figment")
        }) && (path.len() == 1 || path.iter().any(|part| part == "*"))
        || path.first().is_some_and(|first| first == "std")
            && path.iter().any(|part| matches!(part.as_str(), "collections" | "env" | "fs"))
            && path.iter().any(|part| part == "*")
}
fn raw_name(last: &str) -> bool {
    matches!(
        last,
        "Value"
            | "Map"
            | "HashMap"
            | "BTreeMap"
            | "File"
            | "OpenOptions"
            | "Vars"
            | "VarsOs"
            | "Args"
            | "ArgsOs"
            | "Config"
            | "Figment"
            | "RegKey"
            | "var"
            | "var_os"
            | "vars"
            | "vars_os"
            | "args"
            | "args_os"
            | "set_var"
            | "remove_var"
            | "<raw-pointer>"
            | "<type-macro>"
    )
}

/// Inspect every module, inactive target, reexport, associated type and declarative signature.
pub(super) fn check(root: &Path, dependencies: &[Dependency]) -> Result<bool> {
    if !execution_exports::findings_with(root, dependencies, restricted)?.is_empty() {
        return Ok(false);
    }
    let names = crate::execution_exports_names::Names::load(root, dependencies, restricted)?;
    let mut public_templates = std::collections::BTreeSet::new();
    for (module, source) in &names.modules {
        for item in &source.items {
            let syn::Item::Macro(item) = item else { continue };
            if !item.mac.path.is_ident("macro_rules") {
                continue;
            }
            let mut identifiers = std::collections::BTreeSet::new();
            crate::capability_source::macro_identifiers(item.mac.tokens.clone(), &mut identifiers);
            if !identifiers.contains("pub") {
                continue;
            }
            if identifiers.iter().any(|name| raw_name(name)) {
                return Ok(false);
            }
            if let Some(name) = &item.ident {
                let mut path = module.clone();
                path.push(name.to_string());
                public_templates.insert(path);
            }
        }
    }
    for (module, source) in &names.modules {
        for item in &source.items {
            let syn::Item::Macro(item) = item else { continue };
            if item.mac.path.is_ident("macro_rules")
                || !names
                    .resolve(module, &item.mac.path, &std::collections::BTreeSet::new())
                    .iter()
                    .any(|path| public_templates.contains(path))
            {
                continue;
            }
            let mut identifiers = std::collections::BTreeSet::new();
            crate::capability_source::macro_identifiers(item.mac.tokens.clone(), &mut identifiers);
            for identifier in identifiers {
                let path = syn::parse_str::<syn::Path>(&identifier).map_err(|cause| {
                    crate::error::Failure::external("parse configuration macro argument", cause)
                })?;
                if names
                    .resolve(module, &path, &std::collections::BTreeSet::new())
                    .iter()
                    .any(|path| restricted(path))
                {
                    return Ok(false);
                }
            }
        }
    }
    Ok(true)
}
