// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Load execution-owner modules and preserve lexical declarations and imports.

use crate::{
    error::{Failure, Result, read},
    execution_exports_names::Names,
    source::rust_files,
    workspace::Dependency,
};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Path, PathBuf},
};
/// Keep each source module's declarations separate from unrelated same-named bindings.
pub(crate) struct Module {
    pub(crate) items: Vec<syn::Item>,
    pub(crate) file: PathBuf,
    pub(crate) bindings: BTreeMap<String, Vec<Vec<String>>>,
    pub(crate) globs: Vec<Vec<String>>,
    pub(crate) aliases: BTreeMap<String, Vec<syn::ItemType>>,
    pub(crate) declarations: BTreeSet<String>,
}

impl Names {
    /// Seed canonical dependency identities and visit every authored source root.
    pub(crate) fn load(
        root: &Path,
        dependencies: &[Dependency],
        restricted: fn(&[String]) -> bool,
    ) -> Result<Self> {
        let mut index = Self { restricted_path: Some(restricted), ..Self::default() };
        for dependency in dependencies {
            let canonical = dependency.name.replace('-', "_");
            let binding =
                dependency.rename.as_deref().unwrap_or(&dependency.name).replace('-', "_");
            index.externs.insert(binding, canonical);
        }
        let paths = rust_files(root)?;
        for name in ["lib.rs", "main.rs"] {
            let path = root.join("src").join(name);
            if path.is_file() {
                index.file(&path, vec![name.into()], root)?;
            }
        }
        for path in paths {
            if !index.loaded.contains(&path) {
                index.file(&path, vec![format!("source:{}", path.display())], root)?;
            }
        }
        Ok(index)
    }

    /// Parse a source file at its logical module path rather than guessing aliases from filenames.
    fn file(&mut self, path: &Path, key: Vec<String>, root: &Path) -> Result<()> {
        let canonical = path
            .canonicalize()
            .map_err(|error| Failure::external("resolve execution module", error))?;
        let boundary = root
            .canonicalize()
            .map_err(|error| Failure::external("resolve execution owner", error))?;
        if !canonical.starts_with(boundary) {
            return Err(Failure::Policy(format!(
                "execution: module {} leaves its owner",
                path.display()
            )));
        }
        if !self.active.insert(canonical.clone()) {
            return Err(Failure::Policy(format!(
                "execution: recursive module source {} cannot be audited",
                path.display()
            )));
        }
        self.loaded.insert(path.to_owned());
        let parsed = syn::parse_file(&read(path)?)
            .map_err(|error| Failure::external("parse execution exports", error))?;
        let parent = path
            .parent()
            .ok_or_else(|| Failure::Policy("execution module has no parent".into()))?;
        let directory = if matches!(
            path.file_name().and_then(|name| name.to_str()),
            Some("lib.rs" | "main.rs" | "mod.rs")
        ) {
            parent.to_owned()
        } else {
            parent.join(path.file_stem().unwrap_or_default())
        };
        self.items(parsed.items, key, path, &directory, parent, root)?;
        self.active.remove(&canonical);
        Ok(())
    }

    /// Preserve inline modules and explicit module paths, including platform-conditioned branches.
    fn items(
        &mut self,
        items: Vec<syn::Item>,
        key: Vec<String>,
        path: &Path,
        directory: &Path,
        attribute_base: &Path,
        root: &Path,
    ) -> Result<()> {
        let mut module = Module {
            items: items.clone(),
            file: path.to_owned(),
            bindings: BTreeMap::new(),
            globs: Vec::new(),
            aliases: BTreeMap::new(),
            declarations: BTreeSet::new(),
        };
        for item in &items {
            match item {
                syn::Item::Use(item) => module.import(
                    &item.tree,
                    if item.leading_colon.is_some() { vec!["<extern>".into()] } else { Vec::new() },
                ),
                syn::Item::ExternCrate(item) => {
                    module
                        .bindings
                        .entry(
                            item.rename.as_ref().map_or_else(
                                || item.ident.to_string(),
                                |(_, name)| name.to_string(),
                            ),
                        )
                        .or_default()
                        .push(vec![item.ident.to_string()]);
                }
                syn::Item::Type(item) => {
                    module.aliases.entry(item.ident.to_string()).or_default().push(item.clone());
                }
                _ => {}
            }
            if let Some(name) = declaration_name(item) {
                module.declarations.insert(name);
            }
        }
        if let Some(existing) = self.modules.get_mut(&key) {
            existing.merge(module);
        } else {
            self.modules.insert(key.clone(), module);
        }
        for item in items {
            if let syn::Item::Mod(item) = item {
                let mut child = key.clone();
                child.push(item.ident.to_string());
                if let Some((_, items)) = item.content {
                    let child_directory = directory.join(item.ident.to_string());
                    self.items(items, child, path, &child_directory, &child_directory, root)?;
                } else {
                    let mut candidates = BTreeSet::new();
                    for attr in &item.attrs {
                        for meta in attribute_alternatives(&attr.meta) {
                            if let syn::Meta::NameValue(value) = meta
                                && value.path.is_ident("path")
                                && let syn::Expr::Lit(value) = value.value
                                && let syn::Lit::Str(value) = value.lit
                            {
                                candidates.insert(attribute_base.join(value.value()));
                            }
                        }
                    }
                    let flat = directory.join(format!("{}.rs", item.ident));
                    let default = if flat.is_file() {
                        flat
                    } else {
                        directory.join(item.ident.to_string()).join("mod.rs")
                    };
                    if candidates.is_empty() || default.is_file() {
                        candidates.insert(default);
                    }
                    for candidate in candidates {
                        self.file(&candidate, child.clone(), root)?;
                    }
                }
            }
        }
        Ok(())
    }
}

/// Retain conditional attribute payloads without evaluating platform or feature predicates.
pub(crate) fn attribute_alternatives(meta: &syn::Meta) -> Vec<syn::Meta> {
    use syn::parse::Parser;
    let mut result = vec![meta.clone()];
    if let syn::Meta::List(list) = meta
        && list.path.is_ident("cfg_attr")
        && let Ok(items) =
            syn::punctuated::Punctuated::<syn::Meta, syn::Token![,]>::parse_terminated
                .parse2(list.tokens.clone())
    {
        for item in items.iter().skip(1) {
            result.extend(attribute_alternatives(item));
        }
    }
    result
}

/// Recognize a direct or conditionally declared attribute by its exact syntax identity.
pub(crate) fn attribute_contains(meta: &syn::Meta, name: &str) -> bool {
    attribute_alternatives(meta).iter().any(|meta| meta.path().is_ident(name))
}

impl Module {
    /// Audit every platform alternative instead of replacing an earlier same-named module.
    fn merge(&mut self, other: Self) {
        self.items.extend(other.items);
        self.globs.extend(other.globs);
        self.declarations.extend(other.declarations);
        for (name, paths) in other.bindings {
            self.bindings.entry(name).or_default().extend(paths);
        }
        for (name, aliases) in other.aliases {
            self.aliases.entry(name).or_default().extend(aliases);
        }
    }

    /// Flatten Rust's grouped and renamed import syntax without conflating lexical scopes.
    fn import(&mut self, tree: &syn::UseTree, mut prefix: Vec<String>) {
        match tree {
            syn::UseTree::Path(path) => {
                prefix.push(path.ident.to_string());
                self.import(&path.tree, prefix);
            }
            syn::UseTree::Group(group) => {
                for item in &group.items {
                    self.import(item, prefix.clone());
                }
            }
            syn::UseTree::Name(name) => {
                let binding = if name.ident == "self" {
                    prefix.last().cloned().unwrap_or_default()
                } else {
                    prefix.push(name.ident.to_string());
                    name.ident.to_string()
                };
                self.bindings.entry(binding).or_default().push(prefix);
            }
            syn::UseTree::Rename(rename) => {
                if rename.ident != "self" {
                    prefix.push(rename.ident.to_string());
                }
                self.bindings.entry(rename.rename.to_string()).or_default().push(prefix);
            }
            syn::UseTree::Glob(_) => self.globs.push(prefix),
        }
    }
}

/// Recognize declarations that shadow wildcard imports in their own type namespace.
fn declaration_name(item: &syn::Item) -> Option<String> {
    match item {
        syn::Item::Struct(item) => Some(item.ident.to_string()),
        syn::Item::Enum(item) => Some(item.ident.to_string()),
        syn::Item::Union(item) => Some(item.ident.to_string()),
        syn::Item::Trait(item) => Some(item.ident.to_string()),
        syn::Item::Mod(item) => Some(item.ident.to_string()),
        syn::Item::Macro(item) => item.ident.as_ref().map(ToString::to_string),
        _ => None,
    }
}
