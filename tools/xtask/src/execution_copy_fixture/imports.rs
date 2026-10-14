// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Reject cross-parent module imports of the exact producer and consumer source.
use super::*;
use crate::execution_exports_modules::attribute_alternatives;
use std::{collections::BTreeSet, path::PathBuf};
/// Scan every authored parent, including inactive cfg branches, before file-based fixture permission.
/// Canonical identity detects alternate spellings; no imported source grants native authority.
pub(in crate::execution) fn import_findings(
    root: &Path,
    file: &Path,
    source: &str,
) -> Result<Vec<String>> {
    let parsed = syn::parse_file(source)
        .map_err(|cause| Failure::external("parse Report fixture source imports", cause))?;
    let parent =
        file.parent().ok_or_else(|| Failure::Policy("Report source has no parent".into()))?;
    let directory = if matches!(
        file.file_name().and_then(|name| name.to_str()),
        Some("lib.rs" | "main.rs" | "mod.rs")
    ) {
        parent.to_owned()
    } else {
        parent.join(file.file_stem().unwrap_or_default())
    };
    let mut found = BTreeSet::new();
    inspect(root, file, &parsed.items, &directory, parent, false, &mut found)?;
    Ok(found.into_iter().collect())
}
fn inspect(
    root: &Path,
    file: &Path,
    items: &[syn::Item],
    directory: &Path,
    attribute_base: &Path,
    inline: bool,
    found: &mut BTreeSet<String>,
) -> Result<()> {
    for item in items {
        let syn::Item::Mod(module) = item else { continue };
        let mut paths = Vec::new();
        for attribute in &module.attrs {
            for meta in attribute_alternatives(&attribute.meta) {
                if let syn::Meta::NameValue(value) = meta
                    && value.path.is_ident("path")
                    && let syn::Expr::Lit(value) = value.value
                    && let syn::Lit::Str(value) = value.lit
                {
                    paths.push(attribute_base.join(value.value()));
                }
            }
        }
        if let Some((_, children)) = &module.content {
            let child = directory.join(module.ident.to_string());
            inspect(root, file, children, &child, &child, true, found)?;
            continue;
        }
        if paths.is_empty() {
            let flat = directory.join(format!("{}.rs", module.ident));
            paths.push(if flat.is_file() {
                flat
            } else {
                directory.join(module.ident.to_string()).join("mod.rs")
            });
        }
        for path in paths {
            let Some(target) = fixture_target(root, &path) else { continue };
            let (parent, name, feature) = if target == PRODUCER {
                (
                    "crates/sugarsubstitute-desktop/src/report.rs",
                    "copy_fixture",
                    Some("boundary-fixtures"),
                )
            } else {
                (
                    "crates/sugarsubstitute/src/production/reporter_role/runtime/control.rs",
                    "tests",
                    None,
                )
            };
            let expected = root.join(parent).canonicalize().map_err(|cause| {
                Failure::external("resolve original Report fixture parent", cause)
            })?;
            let actual = file
                .canonicalize()
                .map_err(|cause| Failure::external("resolve Report importing source", cause))?;
            if inline || actual != expected || module.ident != name || !gated(module, feature) {
                found.insert(format!("{} imports {target}", module.ident));
            }
        }
    }
    Ok(())
}
fn fixture_target(root: &Path, candidate: &Path) -> Option<&'static str> {
    let candidate: PathBuf = candidate.canonicalize().ok()?;
    [PRODUCER, CONSUMER]
        .into_iter()
        .find(|target| root.join(target).canonicalize().ok().as_ref() == Some(&candidate))
}
