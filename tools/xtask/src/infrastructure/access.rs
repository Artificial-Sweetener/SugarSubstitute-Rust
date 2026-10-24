// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Classify supported filesystem construction, native entry points and raw exported capabilities.
use crate::{capability_source, error::Result, workspace::Dependency};
use std::collections::BTreeSet;
use syn::visit::Visit;
mod native;
mod scripts;
pub(super) use scripts::classify as script;

/// Standard-library file operations remain restricted after imports, aliases and macro expansion.
pub(super) fn restricted(path: &[String]) -> bool {
    let Some(last) = path.last() else { return false };
    if native::known(last) {
        return true;
    }
    if path.first().is_some_and(|name| name == "link_name") && native::posix(last) {
        return true;
    }
    if path
        .iter()
        .position(|name| name == "FileSystem")
        .is_some_and(|index| index + 1 == path.len() || last == "*")
    {
        return true;
    }
    if path.starts_with(&["std".into(), "fs".into()]) {
        return path.get(2).is_some_and(|name| {
            matches!(name.as_str(), "*" | "File" | "OpenOptions")
                || matches!(
                    name.as_str(),
                    "read"
                        | "read_to_string"
                        | "read_dir"
                        | "write"
                        | "copy"
                        | "rename"
                        | "remove_file"
                        | "remove_dir"
                        | "remove_dir_all"
                        | "create_dir"
                        | "create_dir_all"
                        | "canonicalize"
                        | "metadata"
                        | "symlink_metadata"
                        | "hard_link"
                        | "read_link"
                        | "set_permissions"
                )
        });
    }
    path.first().is_some_and(|name| {
        matches!(
            name.as_str(),
            "libc"
                | "rustix"
                | "fs2"
                | "fs4"
                | "fs_extra"
                | "cap_std"
                | "walkdir"
                | "jwalk"
                | "notify"
                | "tempfile"
        )
    }) || path.starts_with(&["tokio".into(), "fs".into()])
        || path.starts_with(&["async_std".into(), "fs".into()])
        || path.starts_with(&["std".into(), "os".into()])
            && path.iter().any(|part| part == "fs")
            && matches!(last.as_str(), "symlink" | "symlink_file" | "symlink_dir" | "*")
        || path.starts_with(&["std".into(), "path".into()]) && observation(last)
}

/// These values expose actual file acquisition or library-owned path output at public boundaries.
pub(super) fn exported(path: &[String]) -> bool {
    path == ["std"]
        || path.starts_with(&["std".into(), "fs".into()])
            && (path.len() == 2
                || path.iter().any(|name| {
                    matches!(name.as_str(), "File" | "OpenOptions" | "ReadDir" | "DirEntry" | "*")
                }))
        || path.first().is_some_and(|name| name == "tiny_skia")
            && (path.len() == 1
                || path
                    .last()
                    .is_some_and(|name| matches!(name.as_str(), "Pixmap" | "PixmapRef" | "*")))
        || path.starts_with(&["std".into(), "os".into()])
            && (path.len() <= 3
                || path.last().is_some_and(|name| {
                    matches!(
                        name.as_str(),
                        "RawFd"
                            | "OwnedFd"
                            | "BorrowedFd"
                            | "RawHandle"
                            | "OwnedHandle"
                            | "BorrowedHandle"
                            | "io"
                            | "fd"
                            | "fs"
                            | "*"
                    )
                }))
        || path.first().is_some_and(|name| matches!(name.as_str(), "cap_std" | "fs2" | "fs4"))
}

/// Catch split macro paths and resolved imported constructor identities without banning ordinary names.
pub(super) fn fragments(names: &BTreeSet<String>) -> bool {
    names.iter().any(|name| native::known(name))
        || names.contains("extern") && names.iter().any(|name| native::posix(name))
        || names.contains("link_name") && names.contains("concat")
        || names.contains("fs")
            && names.iter().any(|name| {
                matches!(
                    name.as_str(),
                    "OpenOptions" | "rename" | "write" | "read_dir" | "create_dir" | "remove_file"
                )
            })
        || names.contains("File")
            && names
                .iter()
                .any(|name| matches!(name.as_str(), "open" | "create" | "create_new" | "options"))
}

/// Resolve renamed Cargo dependencies before applying the same import/macro source checker.
pub(super) fn rust(source: &str, dependencies: &[Dependency]) -> Result<Vec<String>> {
    let bindings: Vec<_> = dependencies
        .iter()
        .filter_map(|dependency| {
            dependency
                .rename
                .as_ref()
                .map(|binding| (binding.replace('-', "_"), dependency.name.replace('-', "_")))
        })
        .collect();
    let mut findings = capability_source::findings_with_literals(
        source,
        restricted,
        fragments,
        |name| matches!(name, "open" | "read" | "write" | "rename" | "File" | "notify"),
        Some(literal),
        &bindings,
    )?;
    let tokens = source
        .parse()
        .map_err(|cause| crate::error::Failure::external("parse filesystem macro tokens", cause))?;
    let mut identifiers = BTreeSet::new();
    capability_source::macro_identifiers(tokens, &mut identifiers);
    if identifiers.contains("macro_rules") && fragments(&identifiers) {
        findings.push("declarative filesystem capability fragments".into());
    }
    if source.contains("save_png") || source.contains("load_png") {
        findings.push("library path-based raster I/O".into());
    }
    let parsed = syn::parse_file(source)
        .map_err(|cause| crate::error::Failure::external("parse filesystem methods", cause))?;
    let mut methods = Methods::default();
    methods.visit_file(&parsed);
    findings.extend(methods.0);
    findings.sort();
    findings.dedup();
    Ok(findings)
}

/// Method syntax cannot conceal platform observations or file-lifetime mutations.
fn observation(name: &str) -> bool {
    matches!(
        name,
        "metadata"
            | "symlink_metadata"
            | "canonicalize"
            | "try_exists"
            | "exists"
            | "read_dir"
            | "set_len"
            | "set_times"
            | "sync_all"
            | "sync_data"
            | "set_permissions"
    )
}

#[derive(Default)]
struct Methods(BTreeSet<String>);
impl<'ast> Visit<'ast> for Methods {
    fn visit_foreign_item_fn(&mut self, function: &'ast syn::ForeignItemFn) {
        if native::posix(&function.sig.ident.to_string()) {
            self.0.insert(format!("native symbol {}", function.sig.ident));
        }
        syn::visit::visit_foreign_item_fn(self, function);
    }

    fn visit_expr_method_call(&mut self, call: &'ast syn::ExprMethodCall) {
        let name = call.method.to_string();
        if observation(&name) {
            self.0.insert(format!("method {name}"));
        }
        syn::visit::visit_expr_method_call(self, call);
    }
    fn visit_attribute(&mut self, attribute: &'ast syn::Attribute) {
        if let syn::Meta::NameValue(value) = &attribute.meta
            && value.path.is_ident("link_name")
            && !matches!(&value.value, syn::Expr::Lit(value) if matches!(value.lit, syn::Lit::Str(_)))
        {
            self.0.insert("computed native linker identity".into());
        }
        syn::visit::visit_attribute(self, attribute);
    }
}

/// Known native/script literals include byte/C strings decoded by the shared Rust scanner.
fn literal(value: &str) -> bool {
    native::known(value.trim_end_matches('\0'))
        || ["node:fs", "fs/promises", "std::fs::", "std::filesystem", "System.IO.File"]
            .iter()
            .any(|identity| value.contains(identity))
}
