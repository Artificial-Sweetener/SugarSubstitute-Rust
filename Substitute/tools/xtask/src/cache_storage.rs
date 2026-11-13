// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Bind the persistent cache registry to one reviewed storage owner and reject known path bypasses.
use crate::{
    catalog::{self, Caches},
    error::{Failure, Result, read, require},
    policy::Architecture,
    source::{resolve, source_files},
};
mod coverage;
mod registry;
mod scripts;
use serde::Deserialize;
use std::{collections::BTreeSet, path::Path};

/// Reviewed implementation of runtime disk admission, retention and native entry custody.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Storage {
    pub(crate) schema_version: u32,
    pub(crate) owner: String,
    pub(crate) control_cache: String,
    pub(crate) entry_source: String,
    pub(crate) registry_source: String,
    pub(crate) script_boundaries: Vec<scripts::Boundary>,
    pub(crate) source_roots: Vec<String>,
    pub(crate) namespace_reviews: Vec<coverage::Review>,
}
/// Require exact namespace parity and reject authored runtime literals that bypass storage routing.
pub(crate) fn check(
    root: &Path,
    architecture: &Architecture,
    caches: &Caches,
    storage: &Storage,
) -> Result<()> {
    require(storage.schema_version == 1, "cache storage policy requires schema one")?;
    let control = caches
        .caches
        .iter()
        .find(|cache| cache.id == storage.control_cache)
        .ok_or_else(|| Failure::Policy("cache storage control must be catalogued".into()))?;
    require(
        control.owner == storage.owner,
        "cache control and implementation must have the same owner",
    )?;
    require(
        control.sources.contains(&storage.entry_source)
            && control.sources.contains(&storage.registry_source),
        "cache storage entry and namespace registry require reviewed ownership",
    )?;
    catalog::reviewed_sources(
        root,
        architecture,
        &control.owner,
        &control.sources,
        &control.source_sha256,
    )?;
    let mut forbidden = registry::names(&read(&resolve(root, &storage.registry_source)?)?)?;
    let directory = registry::directory(&read(&resolve(root, &storage.entry_source)?)?)?;
    require(
        directory == control.namespace,
        "cache control namespace must match the actual scope directory",
    )?;
    require(forbidden.insert(directory), "cache scope and feature namespaces must be distinct")?;
    let expected: BTreeSet<_> = caches.caches.iter().map(|cache| cache.namespace.clone()).collect();
    require(
        forbidden == expected,
        "every persistent cache must use exactly one registered runtime storage namespace",
    )?;
    let module = storage
        .entry_source
        .strip_suffix(".rs")
        .ok_or_else(|| Failure::Policy("cache entry source must be Rust".into()))?;
    let module_prefix = format!("{module}/");
    forbidden.extend(["cache-v1".into(), "webview-v1".into()]);
    scripts::check(root, architecture, &storage.script_boundaries)?;
    let paths = crate::authored_sources::sources(
        root,
        architecture,
        &storage.source_roots,
        scripts::EXTENSIONS,
    )?;
    let mut reviews = coverage::reviews(root, architecture, &storage.namespace_reviews)?;
    for path in paths {
        let relative = path
            .strip_prefix(root)
            .map_err(|cause| Failure::external("resolve cache source", cause))?
            .to_string_lossy()
            .replace('\\', "/");
        if relative == storage.entry_source || relative.starts_with(&module_prefix) {
            require(
                control.sources.contains(&relative),
                format!("{relative}: cache implementation source must be reviewed"),
            )?;
            continue;
        }
        let source = read(&path)?;
        let bypasses = if path
            .extension()
            .and_then(|value| value.to_str())
            .is_some_and(|extension| extension.eq_ignore_ascii_case("rs"))
        {
            let tokens = source.parse::<proc_macro2::TokenStream>().map_err(|cause| {
                Failure::Policy(format!("cache routing: invalid Rust in {relative}: {cause}"))
            })?;
            bypass(tokens, &forbidden)
        } else {
            scripts::namespace_bypass(&source, &forbidden)
        };
        if bypasses && reviews.remove(relative.as_str()) {
            continue;
        }
        require(
            !bypasses,
            format!("{relative}: obtain persistent cache storage through its registered owner"),
        )?;
    }
    require(reviews.is_empty(), "cache: remove unused namespace data reviews")?;
    Ok(())
}
/// Macro token trees, raw strings and constants retain the same known-path rejection behavior.
fn bypass(tokens: proc_macro2::TokenStream, namespaces: &BTreeSet<String>) -> bool {
    tokens.into_iter().any(|token| match token {
        proc_macro2::TokenTree::Group(group) => bypass(group.stream(), namespaces),
        proc_macro2::TokenTree::Literal(value) => {
            let bytes = match syn::parse_str::<syn::Lit>(&value.to_string()) {
                Ok(syn::Lit::Str(value)) => value.value().into_bytes(),
                Ok(syn::Lit::ByteStr(value)) => value.value(),
                Ok(syn::Lit::CStr(value)) => value.value().into_bytes(),
                _ => return false,
            };
            bytes
                .split(|byte| matches!(byte, b'/' | b'\\'))
                .any(|part| known_namespace(part, namespaces))
        }
        _ => false,
    })
}
/// Registered namespaces are ASCII and Windows resolves their directory spellings without case.
fn known_namespace(part: &[u8], namespaces: &BTreeSet<String>) -> bool {
    namespaces.iter().any(|namespace| namespace.as_bytes().eq_ignore_ascii_case(part))
}
#[cfg(test)]
mod token_tests {
    use super::*;
    #[test]
    fn known_cache_path_bypasses_are_rejected_in_constants_macros_and_nested_paths() -> Result<()> {
        let namespaces = BTreeSet::from(["offer-v1".into(), "runtime-cache-v1".into()]);
        for source in [
            "root.join(\"offer-v1\")",
            "const CACHE: &str = r#\"runtime-cache-v1\"#;",
            "cache_path!(root, \"offer-v1\")",
            "root.join(\"runtime-cache-v1/offer-v1\")",
            "cache_path!(root, \"RUNTIME-CACHE-V1/OfFeR-V1\")",
            r#"cache_path!(root, b"runtime-cache-v1")"#,
            r##"cache_path!(root, br#"scope\OFFER-V1\entry"#)"##,
            r#"cache_path!(root, b"\xff/RUNTIME-CACHE-V1")"#,
            r#"cache_path!(root, c"runtime-cache-v1")"#,
            r##"cache_path!(root, cr#"RUNTIME-CACHE-V1"#)"##,
        ] {
            let tokens = source
                .parse()
                .map_err(|cause| Failure::Policy(format!("fixture tokens: {cause}")))?;
            assert!(bypass(tokens, &namespaces), "accepted {source}");
        }
        assert!(!bypass(
            "cache.acquire(Namespace::UpdateOffers, key)"
                .parse()
                .map_err(|cause| Failure::Policy(format!("fixture tokens: {cause}")))?,
            &namespaces
        ));
        Ok(())
    }
}

#[cfg(test)]
mod tests;
