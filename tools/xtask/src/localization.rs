// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Gate known presentation capabilities and single-owner catalog construction.
//!
//! Parsed Rust provenance catches authored literals through local aliases, format macros
//! and wrappers. Native/DOM sinks require exact reviewed adapters with explicit text origins.
//! Script/native token inspection is conservative, not a proof about arbitrary code execution.
use crate::{
    authored_sources, capability_source,
    error::{Failure, Result, read, require},
    policy::Architecture,
    source::{fingerprint, resolve},
    workspace::Workspace,
};
use serde::Deserialize;
use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
};
mod dependencies;
mod html;
mod lexical;
mod registry;
mod rust_copy;
#[cfg(test)]
mod tests;

const EXTENSIONS: &[&str] = &[
    "rs", "js", "jsx", "mjs", "cjs", "ts", "tsx", "mts", "cts", "html", "c", "cc", "cpp", "cxx",
    "h", "hpp", "hxx", "m", "mm", "py", "ps1", "rc", "svg", "cs", "cmake", "gn", "gni", "css",
    "patch",
];

/// One existing catalog and finite reviewed presentation boundaries.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Policy {
    schema_version: u32,
    owner: String,
    registry: String,
    catalog_root: String,
    auxiliary_roots: Vec<String>,
    script_manifests: Vec<String>,
    records: Vec<Record>,
}
/// Review the actual dynamic origin and destination; fingerprints only detect review staleness.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Record {
    path: String,
    source_sha256: String,
    kind: Kind,
    rationale: String,
}
/// Fixtures can deliberately construct rejected text; production adapters must consume owned copy.
#[derive(Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
enum Kind {
    Presentation,
    Native,
    External,
    Fixture,
}

/// Inspect all authored targets/features/platforms and auxiliary languages before compilation.
pub(crate) fn check(
    root: &Path,
    architecture: &Architecture,
    workspace: &Workspace,
    policy: &Policy,
) -> Result<()> {
    require(policy.schema_version == 1, "localization: unsupported schema")?;
    require(
        policy.script_manifests.iter().any(|path| path == "installer/package.json"),
        "localization: missing script dependency coverage",
    )?;
    for relative in &policy.script_manifests {
        dependencies::check(&resolve(root, relative)?)?;
    }
    let owner = architecture
        .owners
        .iter()
        .find(|owner| owner.package == policy.owner)
        .ok_or_else(|| Failure::Policy("localization: unknown catalog owner".into()))?;
    let registry = resolve(root, &policy.registry)?;
    let catalog = resolve(root, &policy.catalog_root)?;
    let locales = registry::identities(&read(&registry)?)?;
    require(
        owner.layer == "domain"
            && policy.registry == format!("{}/src/localization.rs", owner.root)
            && policy.catalog_root == format!("{}/src/localization", owner.root)
            && registry.is_file()
            && catalog.is_dir(),
        "localization: catalog and registry require the existing domain owner",
    )?;
    for package in &workspace.packages {
        for dependency in &package.dependencies {
            require(
                !matches!(
                    dependency.name.as_str(),
                    "fluent" | "fluent-bundle" | "rust-i18n" | "i18n-embed" | "gettext-rs" | "tr"
                ),
                format!(
                    "localization: {} introduces an independent localization dependency {}",
                    package.name, dependency.name
                ),
            )?;
        }
    }
    let mut records = BTreeMap::new();
    for record in &policy.records {
        require(
            records.insert(record.path.as_str(), record).is_none(),
            "localization: duplicate review",
        )?;
        require(
            !record.rationale.trim().is_empty(),
            "localization: review needs text origin rationale",
        )?;
        require(
            fingerprint(root, std::slice::from_ref(&record.path))? == record.source_sha256,
            format!("localization: stale review {}", record.path),
        )?;
    }
    let roots = ["crates", "tools", "native", "installer"].map(String::from);
    let mut extensions = EXTENSIONS.to_vec();
    extensions.push("json");
    let mut files = Vec::new();
    let mut manifests = BTreeSet::new();
    for path in authored_sources::sources(root, architecture, &roots, &extensions)? {
        require(
            architecture.owners.iter().filter(|owner| owner.owns(root, &path)).count() == 1,
            "localization: authored source requires exactly one owner",
        )?;
        if path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.eq_ignore_ascii_case("package.json"))
        {
            manifests.insert(
                path.strip_prefix(root)
                    .map_err(|cause| Failure::external("resolve localization manifest", cause))?
                    .to_string_lossy()
                    .replace('\\', "/"),
            );
        } else if !path
            .extension()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.eq_ignore_ascii_case("json"))
        {
            files.push(path);
        }
    }
    require(
        manifests == policy.script_manifests.iter().cloned().collect()
            && policy.script_manifests.iter().collect::<BTreeSet<_>>().len()
                == policy.script_manifests.len(),
        "localization: script dependency coverage is stale or incomplete",
    )?;
    // These executable source roots must never disappear from the independent layer inventory.
    for required in ["native", "installer/tests"] {
        require(
            policy.auxiliary_roots.iter().any(|root| root == required),
            format!("localization: missing auxiliary coverage {required}"),
        )?;
    }
    let mut used = BTreeSet::new();
    let mut missing = Vec::new();
    for path in files {
        if path == registry || path.starts_with(&catalog) {
            continue;
        }
        let relative = path
            .strip_prefix(root)
            .map_err(|cause| Failure::external("localization source path", cause))?
            .to_string_lossy()
            .replace('\\', "/");
        let source = read(&path)?;
        let extension = path.extension().and_then(|value| value.to_str()).unwrap_or("");
        let rust = extension.eq_ignore_ascii_case("rs");
        let capabilities = if rust {
            capability_source::findings(&source, restricted_path, fragments, |_| false)?
        } else {
            lexical::findings(&source, extension)
        };
        let mut copy = if rust {
            rust_copy::findings(&source, &locales)?
        } else if extension.eq_ignore_ascii_case("html") || extension.eq_ignore_ascii_case("svg") {
            html::findings(&source)?
        } else {
            Vec::new()
        };
        if !rust && registry::parallel_literals(&source, &locales) {
            copy.push("parallel script/native locale inventory".into());
        }
        if !capabilities.is_empty() || !copy.is_empty() {
            let Some(review) = records.get(relative.as_str()) else {
                missing.push(format!(
                    "{relative}: {}",
                    capabilities.iter().chain(&copy).cloned().collect::<Vec<_>>().join(", ")
                ));
                continue;
            };
            require(
                copy.is_empty() || review.kind == Kind::Fixture,
                format!(
                    "localization: {relative} constructs owned copy or a second registry: {}",
                    copy.join(", ")
                ),
            )?;
            used.insert(relative);
        }
    }
    require(
        missing.is_empty(),
        format!("localization: unreviewed presentation routes:\n{}", missing.join("\n")),
    )?;
    let unused: Vec<_> = records.keys().copied().filter(|path| !used.contains(*path)).collect();
    require(
        unused.is_empty(),
        format!(
            "localization: remove unused or out-of-scope presentation reviews:\n{}",
            unused.join("\n")
        ),
    )
}

/// Match operation identity before import aliasing, including reexports and foreign linker names.
fn restricted_path(path: &[String]) -> bool {
    path.iter().any(|name| product_record(name))
        || path.last().is_some_and(|name| rust_sink(name))
        || path.last().is_some_and(|name| name == "*")
            && path
                .iter()
                .any(|name| matches!(name.as_str(), "WindowsAndMessaging" | "Dialogs" | "Shell"))
}
/// Retain typed presentation ownership through associated constructors as well as struct literals.
fn product_record(name: &str) -> bool {
    matches!(
        name,
        "InstallerWindow"
            | "DesktopOptions"
            | "AccessibleControl"
            | "MainCaptionLabels"
            | "MainRecoveryLabels"
    )
}
/// Split macro paths retain their sink identities even when parameterized.
fn fragments(names: &BTreeSet<String>) -> bool {
    names.iter().any(|name| rust_sink(name))
}
/// Browser/global names have ordinary Rust counterparts and are inspected only in their language.
fn rust_sink(name: &str) -> bool {
    sink(name)
        && !matches!(
            name,
            "prompt"
                | "confirm"
                | "alert"
                | "append"
                | "appendChild"
                | "replaceChildren"
                | "insertAdjacentText"
                | "Option"
                | "Notification"
                | "Reflect"
                | "eval"
                | "Function"
        )
}
/// Native, cross-platform and host presentation primitives whose callers require text-origin review.
fn sink(name: &str) -> bool {
    product_record(name)
        || matches!(
            name,
            "MessageBoxA"
                | "set_text"
                | "set_label"
                | "set_markup"
                | "MessageBoxW"
                | "MessageBoxExW"
                | "TaskDialog"
                | "TaskDialogIndirect"
                | "SetWindowTextA"
                | "SetWindowTextW"
                | "SetDlgItemTextW"
                | "SetTitle"
                | "SetOkButtonLabel"
                | "CreateWindowExA"
                | "CreateWindowExW"
                | "TextOutW"
                | "DrawTextW"
                | "with_title"
                | "set_title"
                | "show_startup_failure"
                | "show_startup_conflict"
                | "textContent"
                | "innerText"
                | "innerHTML"
                | "outerHTML"
                | "insertAdjacentHTML"
                | "setAttribute"
                | "createTextNode"
                | "document.write"
                | "alert"
                | "confirm"
                | "prompt"
                | "NSAlert"
                | "gtk_label_set_text"
                | "gtk_message_dialog_new"
                | "g_dgettext"
                | "i18next"
                | "vue-i18n"
                | "react-i18next"
                | "gettext"
                | "GetProcAddress"
                | "dlsym"
                | "append"
                | "appendChild"
                | "replaceChildren"
                | "insertAdjacentText"
                | "Option"
                | "Notification"
                | "setText"
                | "setWindowTitle"
                | "QLabel"
                | "QPushButton"
                | "CAPTION"
                | "LTEXT"
                | "CTEXT"
                | "PUSHBUTTON"
                | "MENUITEM"
                | "Reflect"
                | "eval"
                | "Function"
                | "defineProperty"
                | "defineProperties"
                | "__defineSetter__"
        )
}
