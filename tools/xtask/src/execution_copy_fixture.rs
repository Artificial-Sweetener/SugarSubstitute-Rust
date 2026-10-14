// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Permit one default-off pure Desktop fixture and its exact test-gated receiving consumer.
use crate::{
    error::{Failure, Result, read},
    source::resolve,
};
use std::path::Path;
const CONSUMER: &str =
    "crates/sugarsubstitute/src/production/reporter_role/runtime/control/tests.rs";
const PRODUCER: &str = "crates/sugarsubstitute-desktop/src/report/copy_fixture.rs";
/// Fixture permissions stay separate from preview authority and other production/test source.
pub(super) fn permitted(root: &Path, package: &str, path: &Path) -> Result<bool> {
    let relative = path.strip_prefix(root).unwrap_or(path);
    if package == "sugarsubstitute-desktop" && relative == Path::new(PRODUCER) {
        let manifest = manifest(root, "crates/sugarsubstitute-desktop/Cargo.toml")?;
        let features = manifest.get("features");
        let default_off = features.and_then(|f| f.get("default")).is_none_or(empty_array);
        let pure_feature =
            features.and_then(|f| f.get("boundary-fixtures")).is_some_and(empty_array);
        return Ok(default_off
            && pure_feature
            && module_gated(
                root,
                "crates/sugarsubstitute-desktop/src/report.rs",
                "copy_fixture",
                None,
                Some("boundary-fixtures"),
            )?);
    }
    if package == "sugarsubstitute" && relative == Path::new(CONSUMER) {
        let manifest = manifest(root, "crates/sugarsubstitute/Cargo.toml")?;
        let dev = manifest
            .get("target")
            .and_then(|target| target.get("cfg(windows)"))
            .and_then(|target| target.get("dev-dependencies"))
            .and_then(|deps| deps.get("sugarsubstitute-desktop"));
        return Ok(enables_fixture(dev)
            && !production_enables_fixture(&manifest)
            && module_gated(
                root,
                "crates/sugarsubstitute/src/production/reporter_role/runtime/control.rs",
                "tests",
                Some("control/tests.rs"),
                None,
            )?);
    }
    Ok(false)
}
fn empty_array(value: &toml::Value) -> bool {
    value.as_array().is_some_and(Vec::is_empty)
}
fn enables_fixture(value: Option<&toml::Value>) -> bool {
    value.and_then(|value| value.get("features")).and_then(toml::Value::as_array).is_some_and(
        |features| features.iter().any(|feature| feature.as_str() == Some("boundary-fixtures")),
    )
}
/// Cargo unions target and build dependency features; every production edge remains closed.
fn production_enables_fixture(manifest: &toml::Value) -> bool {
    let enables = |table: &toml::Value| {
        ["dependencies", "build-dependencies"].iter().any(|name| {
            table.get(*name).and_then(toml::Value::as_table).is_some_and(|dependencies| {
                dependencies.iter().any(|(alias, dependency)| {
                    dependency.get("package").and_then(toml::Value::as_str).unwrap_or(alias)
                        == "sugarsubstitute-desktop"
                        && enables_fixture(Some(dependency))
                })
            })
        })
    };
    enables(manifest)
        || manifest
            .get("target")
            .and_then(toml::Value::as_table)
            .is_some_and(|targets| targets.values().any(enables))
}
fn manifest(root: &Path, relative: &str) -> Result<toml::Value> {
    toml::from_str(&read(&resolve(root, relative)?)?)
        .map_err(|cause| Failure::external("parse Report fixture feature ownership", cause))
}
/// Verify the actual enclosing module gate and exact path, never a filename-based test exemption.
fn module_gated(
    root: &Path,
    relative: &str,
    name: &str,
    expected_path: Option<&str>,
    feature: Option<&str>,
) -> Result<bool> {
    let parsed = syn::parse_file(&read(&resolve(root, relative)?)?)
        .map_err(|cause| Failure::external("parse Report fixture module ownership", cause))?;
    if parsed.items.iter().any(|item| {
        let syn::Item::Mod(module) = item else { return false };
        module.ident != name
            && module.attrs.iter().any(|attribute| {
                if let syn::Meta::NameValue(value) = &attribute.meta
                    && value.path.is_ident("path")
                    && let syn::Expr::Lit(literal) = &value.value
                    && let syn::Lit::Str(path) = &literal.lit
                {
                    return path.value() == expected_path.unwrap_or("report/copy_fixture.rs");
                }
                false
            })
    }) {
        return Ok(false);
    }
    Ok(parsed.items.iter().any(|item| {
        let syn::Item::Mod(module) = item else { return false };
        if module.ident != name || module.content.is_some() {
            return false;
        }
        let actual_path = module.attrs.iter().find_map(|attribute| {
            if let syn::Meta::NameValue(value) = &attribute.meta
                && value.path.is_ident("path")
                && let syn::Expr::Lit(literal) = &value.value
                && let syn::Lit::Str(path) = &literal.lit
            {
                return Some(path.value());
            }
            None
        });
        actual_path.as_deref() == expected_path && gated(module, feature)
    }))
}

#[path = "execution_copy_fixture/imports.rs"]
mod imports;
pub(super) use imports::import_findings;

fn gated(module: &syn::ItemMod, feature: Option<&str>) -> bool {
    module.attrs.iter().any(|attribute| {
        if !attribute.path().is_ident("cfg") { return false; }
        let Ok(gate) = attribute.parse_args::<syn::Meta>() else { return false; };
        match (gate, feature) {
            (syn::Meta::Path(path), None) => path.is_ident("test"),
            (syn::Meta::NameValue(value), Some(feature)) if value.path.is_ident("feature") => {
                matches!(&value.value, syn::Expr::Lit(literal) if matches!(&literal.lit, syn::Lit::Str(name) if name.value() == feature))
            }
            _ => false,
        }
    })
}
