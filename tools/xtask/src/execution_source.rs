// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Discover scheduling capability imports and references without scanning prose or literals.

use std::collections::{BTreeMap, BTreeSet};

use proc_macro2::{TokenStream, TokenTree};
use syn::visit::Visit;

use crate::error::{Failure, Result};

/// Identify scheduler packages by canonical Cargo identity, independent of dependency aliases.
pub(crate) fn restricted_crate(name: &str) -> bool {
    matches!(
        name,
        "tokio"
            | "tokio_util"
            | "rayon"
            | "rayon_core"
            | "async_std"
            | "smol"
            | "async_executor"
            | "async_global_executor"
            | "async_io"
            | "blocking"
            | "threadpool"
            | "scheduled_thread_pool"
            | "futures"
            | "futures_executor"
            | "futures_util"
            | "futures_task"
            | "crossbeam"
            | "crossbeam_utils"
            | "crossbeam_channel"
            | "async_channel"
            | "flume"
            | "pollster"
    )
}

/// Retain import aliases and original import paths before inspecting executable token trees.
#[derive(Default)]
struct Imports {
    aliases: BTreeMap<String, BTreeSet<Vec<String>>>,
    paths: Vec<Vec<String>>,
    macros: Vec<(String, BTreeSet<String>)>,
}

impl<'ast> Visit<'ast> for Imports {
    /// Flatten grouped imports so aliases and wildcard imports cannot conceal capability access.
    fn visit_item_use(&mut self, item: &'ast syn::ItemUse) {
        self.collect(&item.tree, Vec::new());
    }

    /// Preserve an explicitly renamed external crate's original identity.
    fn visit_item_extern_crate(&mut self, item: &'ast syn::ItemExternCrate) {
        let canonical = vec![item.ident.to_string()];
        self.paths.push(canonical.clone());
        if let Some((_, alias)) = &item.rename {
            self.aliases.entry(alias.to_string()).or_default().insert(canonical);
        }
    }

    /// Inspect literal imports emitted by declarative macros before expanding their aliases.
    fn visit_macro(&mut self, item: &'ast syn::Macro) {
        self.collect_macro_imports(item.tokens.clone());
        let mut identifiers = BTreeSet::new();
        macro_identifiers(item.tokens.clone(), &mut identifiers);
        self.macros.push((
            item.path.segments.last().map(|segment| segment.ident.to_string()).unwrap_or_default(),
            identifiers,
        ));
    }

    /// Classify renamed foreign functions through their explicit linker identities.
    fn visit_attribute(&mut self, attribute: &'ast syn::Attribute) {
        if let syn::Meta::NameValue(value) = &attribute.meta
            && value.path.is_ident("link_name")
            && let syn::Expr::Lit(literal) = &value.value
            && let syn::Lit::Str(name) = &literal.lit
        {
            self.paths.push(vec![name.value()]);
        }
        syn::visit::visit_attribute(self, attribute);
    }
}

impl Imports {
    /// Parse complete use statements inside macro groups without interpreting arbitrary literals.
    fn collect_macro_imports(&mut self, stream: TokenStream) {
        let tokens: Vec<_> = stream.into_iter().collect();
        for (index, token) in tokens.iter().enumerate() {
            if let TokenTree::Group(group) = token {
                self.collect_macro_imports(group.stream());
            }
            if matches!(token, TokenTree::Ident(name) if name == "use") {
                let statement: TokenStream = tokens[index..]
                    .iter()
                    .take_while(|token| !matches!(token, TokenTree::Punct(p) if p.as_char() == ';'))
                    .cloned()
                    .chain(std::iter::once(TokenTree::Punct(proc_macro2::Punct::new(
                        ';',
                        proc_macro2::Spacing::Alone,
                    ))))
                    .collect();
                if let Ok(item) = syn::parse2::<syn::ItemUse>(statement) {
                    self.collect(&item.tree, Vec::new());
                }
            }
        }
    }

    /// Record each leaf independently while retaining its module prefix.
    fn collect(&mut self, tree: &syn::UseTree, mut prefix: Vec<String>) {
        match tree {
            syn::UseTree::Path(path) => {
                prefix.push(path.ident.to_string());
                self.collect(&path.tree, prefix);
            }
            syn::UseTree::Group(group) => {
                for item in &group.items {
                    self.collect(item, prefix.clone());
                }
            }
            syn::UseTree::Name(name) => {
                if name.ident != "self" {
                    prefix.push(name.ident.to_string());
                }
                self.paths.push(prefix.clone());
                self.aliases.entry(name.ident.to_string()).or_default().insert(prefix);
            }
            syn::UseTree::Rename(rename) => {
                if rename.ident != "self" {
                    prefix.push(rename.ident.to_string());
                }
                self.paths.push(prefix.clone());
                self.aliases.entry(rename.rename.to_string()).or_default().insert(prefix);
            }
            syn::UseTree::Glob(_) => {
                prefix.push("*".into());
                self.paths.push(prefix);
            }
        }
    }

    /// Retain every resolution so scope-local aliases cannot hide a canonical capability.
    fn expand(&self, path: Vec<String>) -> BTreeSet<Vec<String>> {
        let mut resolved = BTreeSet::from([path.clone()]);
        self.resolve_alias(path, BTreeSet::new(), &mut resolved);
        resolved
    }

    /// Stop alias cycles while preserving distinct imports instead of overwriting them.
    fn resolve_alias(
        &self,
        path: Vec<String>,
        mut visited: BTreeSet<String>,
        resolved: &mut BTreeSet<Vec<String>>,
    ) {
        let Some(first) = path.first() else { return };
        if !visited.insert(first.clone()) {
            return;
        }
        if let Some(replacements) = self.aliases.get(first) {
            for replacement in replacements {
                if replacement.first() == Some(first) {
                    continue;
                }
                let mut expanded = replacement.clone();
                expanded.extend_from_slice(&path[1..]);
                resolved.insert(expanded.clone());
                self.resolve_alias(expanded, visited.clone(), resolved);
            }
        }
    }
}

/// Inspect macro identifiers across groups without treating string arguments as executable paths.
fn macro_identifiers(tokens: TokenStream, identifiers: &mut BTreeSet<String>) {
    for token in tokens {
        match token {
            TokenTree::Ident(name) => {
                identifiers.insert(name.to_string().trim_start_matches("r#").to_owned());
            }
            TokenTree::Group(group) => macro_identifiers(group.stream(), identifiers),
            _ => {}
        }
    }
}

/// Reject raw thread/queue ownership and native scheduling entry points outside the adapter.
pub(crate) fn restricted_path(path: &[String]) -> bool {
    let Some(first) = path.first().map(String::as_str) else { return false };
    if restricted_crate(first) {
        return true;
    }
    let parts: Vec<_> = path.iter().map(String::as_str).collect();
    matches!(parts.as_slice(), ["std" | "core", "*"])
        || matches!(parts.as_slice(), ["std", "sync", "*"])
        || parts.starts_with(&["std", "thread"])
        || parts.starts_with(&["std", "sync", "mpsc"])
        || parts.windows(2).any(|pair| pair == ["glib", "MainContext"])
        || path.last().is_some_and(|name| {
            matches!(
                name.as_str(),
                "CreateThread"
                    | "CreateRemoteThread"
                    | "CreateRemoteThreadEx"
                    | "QueueUserWorkItem"
                    | "CreateThreadpoolWork"
                    | "CreateThreadpoolTimer"
                    | "CreateThreadpoolWait"
                    | "CreateThreadpoolIo"
                    | "StartThreadpoolIo"
                    | "SetThreadpoolTimer"
                    | "SetThreadpoolWait"
                    | "SetThreadpoolWaitEx"
                    | "RegisterWaitForSingleObject"
                    | "SubmitThreadpoolWork"
                    | "TrySubmitThreadpoolCallback"
                    | "pthread_create"
                    | "pthread_detach"
                    | "thrd_create"
                    | "_beginthread"
                    | "_beginthreadex"
                    | "dispatch_async"
                    | "dispatch_async_f"
                    | "spawn_blocking"
                    | "spawn_local"
                    | "spawn_future"
                    | "spawn_future_local"
                    | "spawn_scoped"
                    | "spawn_ok"
                    | "spawn_obj"
            )
        })
}

/// Scan groups recursively, including macro definitions and arguments, while ignoring literals.
fn inspect(
    tokens: TokenStream,
    prefix: &[String],
    imports: &Imports,
    found: &mut BTreeSet<String>,
) {
    let tokens: Vec<_> = tokens.into_iter().collect();
    let mut index = 0;
    while index < tokens.len() {
        match &tokens[index] {
            TokenTree::Group(group) => inspect(group.stream(), prefix, imports, found),
            TokenTree::Ident(name) => {
                let mut path = prefix.to_vec();
                path.push(name.to_string().trim_start_matches("r#").to_owned());
                let mut next = index + 1;
                while next + 2 < tokens.len()
                    && matches!(&tokens[next], TokenTree::Punct(p) if p.as_char() == ':')
                    && matches!(&tokens[next + 1], TokenTree::Punct(p) if p.as_char() == ':')
                {
                    let TokenTree::Ident(segment) = &tokens[next + 2] else {
                        if let TokenTree::Group(group) = &tokens[next + 2] {
                            inspect(group.stream(), &path, imports, found);
                            next += 3;
                        } else if matches!(&tokens[next + 2], TokenTree::Punct(p) if p.as_char() == '*')
                        {
                            path.push("*".into());
                            next += 3;
                        }
                        break;
                    };
                    path.push(segment.to_string().trim_start_matches("r#").to_owned());
                    next += 3;
                }
                let aliased = path.first().is_some_and(|name| imports.aliases.contains_key(name));
                for expanded in imports.expand(path) {
                    let ordinary_name = expanded.len() == 1
                        && !aliased
                        && expanded.first().is_some_and(|name| restricted_crate(name));
                    if !ordinary_name && restricted_path(&expanded) {
                        found.insert(expanded.join("::"));
                    }
                }
                index = next;
                continue;
            }
            _ => {}
        }
        index += 1;
    }
}

/// Return deterministic source findings; invalid Rust never receives a clean inspection result.
pub(crate) fn findings(source: &str) -> Result<Vec<String>> {
    let parsed = syn::parse_file(source)
        .map_err(|error| Failure::external("parse execution ownership", error))?;
    let mut imports = Imports::default();
    imports.visit_file(&parsed);
    let mut found = BTreeSet::new();
    for (name, identifiers) in &imports.macros {
        let expanded: BTreeSet<_> = identifiers
            .iter()
            .flat_map(|name| imports.expand(vec![name.clone()]))
            .flatten()
            .collect();
        if expanded.contains("std") && (expanded.contains("thread") || expanded.contains("mpsc")) {
            found.insert(format!("{name}! scheduling capability fragments"));
        }
    }
    for path in &imports.paths {
        for expanded in imports.expand(path.clone()) {
            if restricted_path(&expanded) {
                found.insert(expanded.join("::"));
            }
        }
    }
    let tokens =
        source.parse().map_err(|error| Failure::external("tokenize execution ownership", error))?;
    inspect(tokens, &[], &imports, &mut found);
    Ok(found.into_iter().collect())
}
