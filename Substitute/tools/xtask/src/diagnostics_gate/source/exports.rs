// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Inspect exported routing signatures independently of private formatter construction.
use crate::error::{Failure, Result};
use std::{
    collections::BTreeSet,
    path::{Path, PathBuf},
};
use syn::visit::Visit;
#[derive(Default)]
struct Exports {
    active: bool,
    paths: Vec<Vec<String>>,
    types: Vec<syn::Type>,
    uncertain: bool,
}
impl<'ast> Visit<'ast> for Exports {
    fn visit_foreign_item_fn(&mut self, item: &'ast syn::ForeignItemFn) {
        if matches!(item.vis, syn::Visibility::Public(_)) {
            self.active = true;
            self.visit_signature(&item.sig);
            self.active = false;
        }
    }
    fn visit_foreign_item_static(&mut self, item: &'ast syn::ForeignItemStatic) {
        if matches!(item.vis, syn::Visibility::Public(_)) {
            self.active = true;
            self.visit_type(&item.ty);
            self.active = false;
        }
    }
    fn visit_item_extern_crate(&mut self, item: &'ast syn::ItemExternCrate) {
        if matches!(item.vis, syn::Visibility::Public(_)) {
            self.paths.push(vec![String::new(), item.ident.to_string()]);
        }
    }
    fn visit_item_const(&mut self, item: &'ast syn::ItemConst) {
        if matches!(item.vis, syn::Visibility::Public(_)) {
            self.active = true;
            self.visit_type(&item.ty);
            self.active = false;
        }
    }
    fn visit_item_static(&mut self, item: &'ast syn::ItemStatic) {
        if matches!(item.vis, syn::Visibility::Public(_)) {
            self.active = true;
            self.visit_type(&item.ty);
            self.active = false;
        }
    }
    fn visit_item_union(&mut self, item: &'ast syn::ItemUnion) {
        if matches!(item.vis, syn::Visibility::Public(_)) {
            for field in &item.fields.named {
                if matches!(field.vis, syn::Visibility::Public(_)) {
                    self.active = true;
                    self.visit_type(&field.ty);
                    self.active = false;
                }
            }
        }
    }
    fn visit_type(&mut self, ty: &'ast syn::Type) {
        if self.active {
            self.types.push(ty.clone());
        }
        syn::visit::visit_type(self, ty);
    }
    fn visit_type_macro(&mut self, item: &'ast syn::TypeMacro) {
        self.uncertain |= self.active;
        syn::visit::visit_type_macro(self, item);
    }
    fn visit_impl_item_fn(&mut self, item: &'ast syn::ImplItemFn) {
        if matches!(item.vis, syn::Visibility::Public(_)) {
            self.active = true;
            self.visit_signature(&item.sig);
            self.active = false;
        }
    }
    fn visit_item_struct(&mut self, item: &'ast syn::ItemStruct) {
        if matches!(item.vis, syn::Visibility::Public(_)) {
            for field in &item.fields {
                if matches!(field.vis, syn::Visibility::Public(_)) {
                    self.active = true;
                    self.visit_type(&field.ty);
                    self.active = false;
                }
            }
        }
    }
    fn visit_item_enum(&mut self, item: &'ast syn::ItemEnum) {
        if matches!(item.vis, syn::Visibility::Public(_)) {
            self.active = true;
            for variant in &item.variants {
                for field in &variant.fields {
                    self.visit_type(&field.ty);
                }
            }
            self.active = false;
        }
    }
    fn visit_item_trait(&mut self, item: &'ast syn::ItemTrait) {
        if matches!(item.vis, syn::Visibility::Public(_)) {
            self.active = true;
            for bound in &item.supertraits {
                self.visit_type_param_bound(bound);
            }
            for member in &item.items {
                match member {
                    syn::TraitItem::Fn(method) => self.visit_signature(&method.sig),
                    syn::TraitItem::Type(ty) => {
                        for bound in &ty.bounds {
                            self.visit_type_param_bound(bound);
                        }
                        if let Some((_, ty)) = &ty.default {
                            self.visit_type(ty);
                        }
                    }
                    syn::TraitItem::Const(value) => self.visit_type(&value.ty),
                    syn::TraitItem::Macro(_) => self.uncertain = true,
                    _ => {}
                }
            }
            self.active = false;
        }
    }
    fn visit_item_fn(&mut self, item: &'ast syn::ItemFn) {
        if matches!(item.vis, syn::Visibility::Public(_)) {
            self.active = true;
            self.visit_signature(&item.sig);
            self.active = false;
        }
    }
    fn visit_item_type(&mut self, item: &'ast syn::ItemType) {
        if matches!(item.vis, syn::Visibility::Public(_)) {
            self.active = true;
            self.visit_type(&item.ty);
            self.active = false;
        }
    }
    fn visit_item_use(&mut self, item: &'ast syn::ItemUse) {
        if matches!(item.vis, syn::Visibility::Public(_)) {
            fn collect(tree: &syn::UseTree, mut prefix: Vec<String>, paths: &mut Vec<Vec<String>>) {
                match tree {
                    syn::UseTree::Path(path) => {
                        prefix.push(path.ident.to_string());
                        collect(&path.tree, prefix, paths);
                    }
                    syn::UseTree::Name(name) => {
                        if name.ident != "self" {
                            prefix.push(name.ident.to_string());
                        }
                        paths.push(prefix);
                    }
                    syn::UseTree::Rename(rename) => {
                        prefix.push(rename.ident.to_string());
                        paths.push(prefix);
                    }
                    syn::UseTree::Group(group) => {
                        for tree in &group.items {
                            collect(tree, prefix.clone(), paths);
                        }
                    }
                    syn::UseTree::Glob(_) => {
                        prefix.push("*".into());
                        paths.push(prefix);
                    }
                }
            }
            collect(
                &item.tree,
                if item.leading_colon.is_some() { vec![String::new()] } else { Vec::new() },
                &mut self.paths,
            );
        }
    }
    fn visit_path(&mut self, path: &'ast syn::Path) {
        if self.active {
            let mut parts =
                if path.leading_colon.is_some() { vec![String::new()] } else { Vec::new() };
            parts.extend(path.segments.iter().map(|segment| segment.ident.to_string()));
            self.paths.push(parts);
        }
        syn::visit::visit_path(self, path);
    }
}
/// Reject public raw subscriber signatures and reexports while permitting the owner's private formatter.
pub(crate) fn public_raw_exports(text: &str) -> Result<bool> {
    let file = syn::parse_file(text)
        .map_err(|cause| Failure::external("parse diagnostic exports", cause))?;
    let mut exports = Exports::default();
    exports.visit_file(&file);
    Ok(exports.uncertain
        || crate::capability_source::resolved_paths_match(text, &exports.paths, |path| {
            raw_path(path)
                || path.last().is_some_and(|last| {
                    matches!(last.as_str(), "Subscriber" | "Dispatch" | "MakeWriter")
                })
        })?)
}

/// Follow cross-file module aliases and associated types through the existing declared-API resolver.
pub(crate) fn crate_public_raw_exports(
    root: &Path,
    dependencies: &[crate::workspace::Dependency],
) -> Result<BTreeSet<PathBuf>> {
    let names = crate::execution_exports_names::Names::load(root, dependencies, |path| {
        path.first().is_some_and(|first| first == "tracing" || first == "tracing_subscriber")
    })?;
    let mut found = BTreeSet::new();
    let parameters = BTreeSet::new();
    for (key, module) in &names.modules {
        let mut exports = Exports::default();
        for item in &module.items {
            exports.visit_item(item);
        }
        let mut resolved = BTreeSet::new();
        for mut parts in exports.paths {
            let glob = parts.last().is_some_and(|part| part == "*");
            if glob {
                parts.pop();
            }
            let path = syn::parse_str::<syn::Path>(&parts.join("::"))
                .map_err(|cause| Failure::external("parse exported diagnostic path", cause))?;
            resolved.extend(names.resolve(key, &path, &parameters).into_iter().map(|mut path| {
                if glob {
                    path.push("*".into());
                }
                path
            }));
        }
        for ty in exports.types {
            resolved.extend(names.resolve_type(key, &ty, &parameters));
        }
        if exports.uncertain
            || resolved.iter().any(|path| {
                path.first().is_some_and(|first| first == "<type-macro>" || raw_path(path))
            })
        {
            found.insert(module.file.clone());
        }
    }
    Ok(found)
}
/// Logging modules and constructors expose authority; level/metadata values remain inert data.
fn raw_path(path: &[String]) -> bool {
    let path = if path.first().is_some_and(String::is_empty) { &path[1..] } else { path };
    path.first().is_some_and(|first| {
        first == "tracing_subscriber"
            || first == "tracing"
                && (path.len() == 1
                    || path.iter().any(|part| {
                        matches!(
                            part.as_str(),
                            "Subscriber"
                                | "Dispatch"
                                | "subscriber"
                                | "dispatcher"
                                | "set_global_default"
                                | "set_default"
                                | "with_default"
                                | "*"
                        )
                    }))
    })
}
