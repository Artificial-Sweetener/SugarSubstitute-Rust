// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Resolve execution API names within their declaring modules and Cargo dependency identities.

use crate::{execution_exports_modules::Module, execution_source};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::PathBuf,
};
use syn::visit::Visit;

/// Index authored modules without compiling, evaluating platform conditions or expanding macros.
#[derive(Default)]
pub(crate) struct Names {
    pub(crate) modules: BTreeMap<Vec<String>, Module>,
    pub(crate) externs: BTreeMap<String, String>,
    pub(crate) loaded: BTreeSet<PathBuf>,
    pub(crate) active: BTreeSet<PathBuf>,
}

impl Names {
    /// Resolve aliases and nested generic type paths without leaking another module's bindings.
    pub(crate) fn resolve(
        &self,
        module: &[String],
        path: &syn::Path,
        parameters: &BTreeSet<String>,
    ) -> BTreeSet<Vec<String>> {
        self.resolve_with(module, path, parameters, &mut BTreeSet::new())
    }

    /// Keep one alias/projection traversal guard across recursive implementation lookup.
    pub(crate) fn resolve_with(
        &self,
        module: &[String],
        path: &syn::Path,
        parameters: &BTreeSet<String>,
        visited: &mut BTreeSet<Vec<String>>,
    ) -> BTreeSet<Vec<String>> {
        self.expand(module, path_parts(path), parameters, visited)
    }

    /// Follow aliases and explicit associated-type projections within a declared type.
    pub(crate) fn resolve_type(
        &self,
        module: &[String],
        ty: &syn::Type,
        parameters: &BTreeSet<String>,
    ) -> BTreeSet<Vec<String>> {
        let mut paths = Paths::default();
        paths.visit_type(ty);
        self.expand_paths(module, paths, parameters, &mut BTreeSet::new())
    }

    /// Preserve type-macro uncertainty and stop associated-type projection cycles.
    fn expand_paths(
        &self,
        module: &[String],
        paths: Paths,
        parameters: &BTreeSet<String>,
        visited: &mut BTreeSet<Vec<String>>,
    ) -> BTreeSet<Vec<String>> {
        let mut found = BTreeSet::new();
        if paths.raw_pointer {
            found.insert(vec!["<raw-pointer>".into()]);
        }
        for path in paths.type_macros {
            found.insert(vec!["<type-macro>".into(), path]);
        }
        for path in paths.paths {
            found.extend(self.expand(module, path_parts(&path), parameters, &mut visited.clone()));
        }
        for projection in paths.projections {
            let mut identity = vec!["<projection>".into()];
            identity.extend(module.to_vec());
            identity.extend(path_parts(&projection.path));
            if let Some(qself) = &projection.qself {
                let mut self_paths = Paths::default();
                self_paths.visit_type(&qself.ty);
                for path in self_paths.paths {
                    identity.extend(path_parts(&path));
                }
            }
            if !visited.insert(identity) {
                continue;
            }
            for (key, ty) in self.projected_types(module, &projection, &mut visited.clone()) {
                let mut nested = Paths::default();
                nested.visit_type(&ty);
                found.extend(self.expand_paths(&key, nested, parameters, &mut visited.clone()));
            }
        }
        found
    }

    /// Follow module-qualified aliases with a cycle guard and conservative restricted-glob handling.
    fn expand(
        &self,
        module: &[String],
        mut parts: Vec<String>,
        parameters: &BTreeSet<String>,
        visited: &mut BTreeSet<Vec<String>>,
    ) -> BTreeSet<Vec<String>> {
        let Some(first) = parts.first().cloned() else { return BTreeSet::from([module.to_vec()]) };
        if parameters.contains(&first) {
            return BTreeSet::new();
        }
        let mut location = module.to_vec();
        let explicit_local = matches!(first.as_str(), "crate" | "self" | "Self" | "super");
        match first.as_str() {
            "<extern>" => {
                parts.remove(0);
                if let Some(canonical) = parts.first().and_then(|first| self.externs.get(first)) {
                    parts[0] = canonical.clone();
                }
                return BTreeSet::from([parts]);
            }
            "crate" => {
                location.truncate(1);
                parts.remove(0);
            }
            "self" | "Self" => {
                parts.remove(0);
            }
            "super" => {
                while parts.first().is_some_and(|part| part == "super") {
                    location.pop();
                    parts.remove(0);
                }
            }
            _ => {}
        }
        let Some(name) = parts.first().cloned() else { return BTreeSet::new() };
        let mut identity = location.clone();
        identity.extend(parts.clone());
        if !visited.insert(identity.clone()) {
            return BTreeSet::new();
        }
        let Some(scope) = self.modules.get(&location) else { return BTreeSet::from([identity]) };
        let mut alternatives = BTreeSet::new();
        let mut child = location.clone();
        child.push(name.clone());
        if parts.len() > 1 && self.modules.contains_key(&child) {
            alternatives.extend(self.expand(
                &child,
                parts[1..].to_vec(),
                parameters,
                &mut visited.clone(),
            ));
        }
        let mut binding_identity = location.clone();
        binding_identity.extend(["<binding>".into(), name.clone()]);
        let binding_available = visited.insert(binding_identity);
        if let Some(bindings) = scope.bindings.get(&name)
            && binding_available
        {
            for binding in bindings {
                if binding.as_slice() == [name.as_str()] {
                    let root = self.externs.get(&name).cloned().unwrap_or_else(|| name.clone());
                    let mut external = vec![root];
                    external.extend_from_slice(&parts[1..]);
                    alternatives.insert(external);
                    continue;
                }
                let mut replacement = binding.clone();
                replacement.extend_from_slice(&parts[1..]);
                alternatives.extend(self.expand(
                    &location,
                    replacement,
                    parameters,
                    &mut visited.clone(),
                ));
            }
        }
        if let Some(aliases) = scope.aliases.get(&name)
            && binding_available
        {
            for alias in aliases {
                let mut types = Paths::default();
                types.visit_type(&alias.ty);
                types.visit_generics(&alias.generics);
                let mut alias_parameters = parameters.clone();
                alias_parameters.extend(generic_names(&alias.generics));
                alternatives.extend(self.expand_paths(
                    &location,
                    types,
                    &alias_parameters,
                    &mut visited.clone(),
                ));
            }
        }
        if scope.bindings.contains_key(&name) || scope.aliases.contains_key(&name) {
            return alternatives;
        }
        if self.modules.contains_key(&child) {
            visited.remove(&identity);
            return self.expand(&child, parts[1..].to_vec(), parameters, visited);
        }
        if scope.declarations.contains(&name) || prelude(&name) {
            return BTreeSet::from([identity]);
        }
        if !explicit_local {
            if let Some(canonical) = self.externs.get(&first) {
                parts[0] = canonical.clone();
                return BTreeSet::from([parts]);
            }
            if matches!(first.as_str(), "std" | "core" | "alloc")
                || execution_source::restricted_crate(&first)
            {
                return BTreeSet::from([parts]);
            }
        }
        let mut found = BTreeSet::new();
        for glob in &scope.globs {
            for mut prefix in self.expand(&location, glob.clone(), parameters, &mut visited.clone())
            {
                if self.modules.contains_key(&prefix) {
                    found.extend(self.expand(
                        &prefix,
                        parts.clone(),
                        parameters,
                        &mut visited.clone(),
                    ));
                } else {
                    prefix.push("*".into());
                    prefix.extend(parts.clone());
                    found.insert(prefix);
                }
            }
        }
        if found.is_empty() {
            found.insert(identity);
        }
        found
    }
}

/// Retain an explicit external root when a local module shadows the same crate name.
fn path_parts(path: &syn::Path) -> Vec<String> {
    let mut parts = Vec::new();
    if path.leading_colon.is_some() {
        parts.push("<extern>".into());
    }
    parts.extend(path.segments.iter().map(|segment| segment.ident.to_string()));
    parts
}

/// Collect paths nested inside type aliases and generic defaults without examining implementation bodies.
#[derive(Default)]
struct Paths {
    paths: Vec<syn::Path>,
    type_macros: Vec<String>,
    projections: Vec<syn::TypePath>,
    raw_pointer: bool,
}
impl<'ast> Visit<'ast> for Paths {
    /// Preserve raw capability shape through private aliases and associated-type projections.
    fn visit_type_ptr(&mut self, item: &'ast syn::TypePtr) {
        self.raw_pointer = true;
        syn::visit::visit_type_ptr(self, item);
    }
    /// Retain explicitly qualified associated types for matching authored implementation lookup.
    fn visit_type_path(&mut self, item: &'ast syn::TypePath) {
        if item.qself.is_some() {
            self.projections.push(item.clone());
        }
        syn::visit::visit_type_path(self, item);
    }
    /// Preserve unanalyzable type expansions reached through private aliases.
    fn visit_type_macro(&mut self, item: &'ast syn::TypeMacro) {
        self.type_macros.push(path_parts(&item.mac.path).join("::"));
    }
    fn visit_path(&mut self, path: &'ast syn::Path) {
        self.paths.push(path.clone());
        syn::visit::visit_path(self, path);
    }
}

/// Preserve generic parameters so a locally bound type cannot be mistaken for an imported capability.
pub(crate) fn generic_names(generics: &syn::Generics) -> BTreeSet<String> {
    generics
        .params
        .iter()
        .filter_map(|parameter| match parameter {
            syn::GenericParam::Type(value) => Some(value.ident.to_string()),
            syn::GenericParam::Const(value) => Some(value.ident.to_string()),
            _ => None,
        })
        .collect()
}

/// Exclude language primitives and the fixed standard prelude from unresolved scheduler globs.
fn prelude(name: &str) -> bool {
    matches!(
        name,
        "bool"
            | "char"
            | "str"
            | "u8"
            | "u16"
            | "u32"
            | "u64"
            | "u128"
            | "usize"
            | "i8"
            | "i16"
            | "i32"
            | "i64"
            | "i128"
            | "isize"
            | "f32"
            | "f64"
            | "Self"
            | "Send"
            | "Sync"
            | "Sized"
            | "Copy"
            | "Clone"
            | "Drop"
            | "Default"
            | "Option"
            | "Result"
            | "Box"
            | "Vec"
            | "String"
            | "Into"
            | "From"
            | "AsRef"
            | "AsMut"
            | "Iterator"
            | "IntoIterator"
            | "Fn"
            | "FnMut"
            | "FnOnce"
    )
}
