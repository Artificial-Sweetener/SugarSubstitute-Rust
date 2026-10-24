// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Reject known raw configuration/file types in the document owner's public API.
use crate::error::{Failure, Result};
use std::collections::BTreeSet;
use syn::visit::Visit;

fn raw(name: &str) -> bool {
    matches!(
        name,
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
    )
}
#[derive(Default)]
struct Types {
    aliases: BTreeSet<String>,
}
impl Types {
    fn imported(&mut self, tree: &syn::UseTree) {
        match tree {
            syn::UseTree::Path(path) => self.imported(&path.tree),
            syn::UseTree::Group(group) => {
                for item in &group.items {
                    self.imported(item);
                }
            }
            syn::UseTree::Rename(rename) if raw(&rename.ident.to_string()) => {
                self.aliases.insert(rename.rename.to_string());
            }
            _ => {}
        }
    }
    fn contains(&self, ty: &syn::Type) -> bool {
        struct Probe<'a> {
            types: &'a Types,
            found: bool,
        }
        impl<'ast> Visit<'ast> for Probe<'_> {
            fn visit_type_path(&mut self, path: &'ast syn::TypePath) {
                self.found |= path.path.segments.last().is_some_and(|segment| {
                    raw(&segment.ident.to_string())
                        || self.types.aliases.contains(&segment.ident.to_string())
                });
                syn::visit::visit_type_path(self, path);
            }
        }
        let mut probe = Probe { types: self, found: false };
        probe.visit_type(ty);
        probe.found
    }
    fn returns_raw(&self, signature: &syn::Signature) -> bool {
        matches!(&signature.output, syn::ReturnType::Type(_, ty) if self.contains(ty))
    }
}
impl<'ast> Visit<'ast> for Types {
    fn visit_item_use(&mut self, item: &'ast syn::ItemUse) {
        self.imported(&item.tree);
    }
    fn visit_item_type(&mut self, item: &'ast syn::ItemType) {
        if self.contains(&item.ty) {
            self.aliases.insert(item.ident.to_string());
        }
        syn::visit::visit_item_type(self, item);
    }
}
struct Exports<'a> {
    types: &'a Types,
    rejected: bool,
}
impl<'ast> Visit<'ast> for Exports<'_> {
    fn visit_macro(&mut self, item: &'ast syn::Macro) {
        fn restricted(tokens: proc_macro2::TokenStream, types: &Types) -> bool {
            tokens.into_iter().any(|token| match token {
                proc_macro2::TokenTree::Ident(name) => {
                    raw(&name.to_string()) || types.aliases.contains(&name.to_string())
                }
                proc_macro2::TokenTree::Group(group) => restricted(group.stream(), types),
                _ => false,
            })
        }
        self.rejected |= restricted(item.tokens.clone(), self.types);
    }
    fn visit_item_const(&mut self, item: &'ast syn::ItemConst) {
        self.rejected |=
            matches!(item.vis, syn::Visibility::Public(_)) && self.types.contains(&item.ty);
        syn::visit::visit_item_const(self, item);
    }
    fn visit_item_static(&mut self, item: &'ast syn::ItemStatic) {
        self.rejected |=
            matches!(item.vis, syn::Visibility::Public(_)) && self.types.contains(&item.ty);
        syn::visit::visit_item_static(self, item);
    }
    fn visit_item_enum(&mut self, item: &'ast syn::ItemEnum) {
        self.rejected |= matches!(item.vis, syn::Visibility::Public(_))
            && item
                .variants
                .iter()
                .any(|variant| variant.fields.iter().any(|field| self.types.contains(&field.ty)));
        syn::visit::visit_item_enum(self, item);
    }
    fn visit_trait_item_fn(&mut self, item: &'ast syn::TraitItemFn) {
        self.rejected |= self.types.returns_raw(&item.sig);
        syn::visit::visit_trait_item_fn(self, item);
    }
    fn visit_item_fn(&mut self, item: &'ast syn::ItemFn) {
        self.rejected |=
            matches!(item.vis, syn::Visibility::Public(_)) && self.types.returns_raw(&item.sig);
        syn::visit::visit_item_fn(self, item);
    }
    fn visit_impl_item_fn(&mut self, item: &'ast syn::ImplItemFn) {
        self.rejected |=
            matches!(item.vis, syn::Visibility::Public(_)) && self.types.returns_raw(&item.sig);
        syn::visit::visit_impl_item_fn(self, item);
    }
    fn visit_item_type(&mut self, item: &'ast syn::ItemType) {
        self.rejected |=
            matches!(item.vis, syn::Visibility::Public(_)) && self.types.contains(&item.ty);
    }
    fn visit_field(&mut self, item: &'ast syn::Field) {
        self.rejected |=
            matches!(item.vis, syn::Visibility::Public(_)) && self.types.contains(&item.ty);
    }
    fn visit_item_use(&mut self, item: &'ast syn::ItemUse) {
        if matches!(item.vis, syn::Visibility::Public(_)) {
            let mut types = Types::default();
            types.imported(&item.tree);
            self.rejected |= !types.aliases.is_empty() || use_raw(&item.tree);
        }
    }
}
fn use_raw(tree: &syn::UseTree) -> bool {
    match tree {
        syn::UseTree::Path(path) => use_raw(&path.tree),
        syn::UseTree::Name(name) => raw(&name.ident.to_string()),
        syn::UseTree::Rename(rename) => raw(&rename.ident.to_string()),
        syn::UseTree::Group(group) => group.items.iter().any(use_raw),
        syn::UseTree::Glob(_) => true,
    }
}
/// Inspect inactive targets and local aliases. This is known-type inspection, not
/// Rust name resolution or proof of arbitrary opaque wrapper behavior.
pub(super) fn check(source: &str) -> Result<bool> {
    let parsed = syn::parse_file(source)
        .map_err(|cause| Failure::external("parse configuration exports", cause))?;
    let mut types = Types::default();
    let mut count = usize::MAX;
    while count != types.aliases.len() {
        count = types.aliases.len();
        types.visit_file(&parsed);
    }
    let mut exports = Exports { types: &types, rejected: false };
    exports.visit_file(&parsed);
    Ok(!exports.rejected)
}
