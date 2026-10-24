// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Resolve publicly usable declarations and explicit associated-type projections.

use crate::execution_exports_names::{Names, generic_names};
use std::collections::BTreeSet;

impl Names {
    /// Resolve a qualified associated type against matching authored implementations.
    pub(crate) fn projected_types(
        &self,
        module: &[String],
        ty: &syn::TypePath,
        visited: &mut BTreeSet<Vec<String>>,
    ) -> Vec<(Vec<String>, syn::Type)> {
        let Some(qself) = &ty.qself else { return Vec::new() };
        let syn::Type::Path(self_ty) = qself.ty.as_ref() else { return Vec::new() };
        if qself.position == 0 {
            return Vec::new();
        }
        let self_names =
            self.resolve_with(module, &self_ty.path, &BTreeSet::new(), &mut visited.clone());
        let mut trait_path = ty.path.clone();
        trait_path.segments = trait_path.segments.into_iter().take(qself.position).collect();
        let trait_names =
            self.resolve_with(module, &trait_path, &BTreeSet::new(), &mut visited.clone());
        let Some(associated) = ty.path.segments.last() else { return Vec::new() };
        let mut found = Vec::new();
        for (key, scope) in &self.modules {
            for item in &scope.items {
                let syn::Item::Impl(item) = item else { continue };
                let Some((_, path, _)) = &item.trait_ else { continue };
                let syn::Type::Path(candidate) = item.self_ty.as_ref() else { continue };
                let parameters = generic_names(&item.generics);
                if self
                    .resolve_with(key, &candidate.path, &parameters, &mut visited.clone())
                    .is_disjoint(&self_names)
                    || self
                        .resolve_with(key, path, &parameters, &mut visited.clone())
                        .is_disjoint(&trait_names)
                {
                    continue;
                }
                for member in &item.items {
                    if let syn::ImplItem::Type(member) = member
                        && member.ident == associated.ident
                    {
                        found.push((key.clone(), member.ty.clone()));
                    }
                }
            }
        }
        found
    }
    /// Determine whether trait projections belong to an externally declared owned type.
    pub(crate) fn public_type(
        &self,
        module: &[String],
        ty: &syn::Type,
        parameters: &BTreeSet<String>,
    ) -> bool {
        match ty {
            syn::Type::Path(ty) => self.public_declaration(module, &ty.path, false, parameters),
            syn::Type::Reference(ty) => self.public_type(module, &ty.elem, parameters),
            syn::Type::Ptr(ty) => self.public_type(module, &ty.elem, parameters),
            syn::Type::Slice(ty) => self.public_type(module, &ty.elem, parameters),
            syn::Type::Array(ty) => self.public_type(module, &ty.elem, parameters),
            syn::Type::Paren(ty) => self.public_type(module, &ty.elem, parameters),
            syn::Type::Group(ty) => self.public_type(module, &ty.elem, parameters),
            syn::Type::Tuple(ty) => {
                ty.elems.iter().any(|ty| self.public_type(module, ty, parameters))
            }
            _ => false,
        }
    }

    /// Public local traits expose implementations even on primitive or reference self types.
    pub(crate) fn public_trait(
        &self,
        module: &[String],
        path: &syn::Path,
        parameters: &BTreeSet<String>,
    ) -> bool {
        self.public_declaration(module, path, true, parameters)
    }

    /// Resolve declaration visibility independently from its enclosing private module.
    fn public_declaration(
        &self,
        module: &[String],
        path: &syn::Path,
        trait_only: bool,
        parameters: &BTreeSet<String>,
    ) -> bool {
        self.resolve(module, path, parameters).iter().any(|path| {
            let Some((name, parent)) = path.split_last() else { return false };
            self.modules.get(parent).is_some_and(|module| {
                module.items.iter().any(|item| match item {
                    syn::Item::Struct(item) if !trait_only => {
                        item.ident == name && matches!(item.vis, syn::Visibility::Public(_))
                    }
                    syn::Item::Enum(item) if !trait_only => {
                        item.ident == name && matches!(item.vis, syn::Visibility::Public(_))
                    }
                    syn::Item::Type(item) if !trait_only => {
                        item.ident == name && matches!(item.vis, syn::Visibility::Public(_))
                    }
                    syn::Item::Union(item) if !trait_only => {
                        item.ident == name && matches!(item.vis, syn::Visibility::Public(_))
                    }
                    syn::Item::Trait(item) if trait_only => {
                        item.ident == name && matches!(item.vis, syn::Visibility::Public(_))
                    }
                    _ => false,
                })
            })
        })
    }

    /// Locate a locally resolved macro re-export at its definition's lexical scope.
    pub(crate) fn macro_items(&self, path: &[String]) -> Vec<(&[String], &syn::ItemMacro)> {
        let Some((name, parent)) = path.split_last() else { return Vec::new() };
        self.modules
            .iter()
            .flat_map(|(key, module)| {
                module.items.iter().filter_map(move |item| match item {
                    syn::Item::Macro(item)
                        if item.ident.as_ref().is_some_and(|ident| ident == name)
                            && (key == parent
                                || (parent.len() == 1
                                    && key.first() == parent.first()
                                    && crate::execution_exports_macros::exported(&item.attrs))) =>
                    {
                        Some((key.as_slice(), item))
                    }
                    _ => None,
                })
            })
            .collect()
    }

    /// Find helper signatures emitted by public macros without inspecting implementation bodies.
    pub(crate) fn functions(&self, path: &[String]) -> Vec<&syn::Signature> {
        let Some((name, parent)) = path.split_last() else { return Vec::new() };
        let Some(module) = self.modules.get(parent) else { return Vec::new() };
        module
            .items
            .iter()
            .filter_map(|item| match item {
                syn::Item::Fn(item) if item.sig.ident == name => Some(&item.sig),
                _ => None,
            })
            .collect()
    }
}
