// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Reject scheduler capabilities in the execution owner's declared public API.

use crate::{
    error::Result,
    execution_exports_macros,
    execution_exports_names::{Names, generic_names},
    execution_source,
    workspace::Dependency,
};
use std::{collections::BTreeSet, path::Path};
use syn::visit::Visit;

/// Inspect all declared public boundaries while retaining private scheduler implementations.
pub(crate) fn findings(root: &Path, dependencies: &[Dependency]) -> Result<Vec<String>> {
    let names = Names::load(
        root,
        dependencies,
        execution_source::restricted_path,
        execution_source::restricted_crate,
    )?;
    let mut found = BTreeSet::new();
    for (key, module) in &names.modules {
        let mut audit = Audit {
            names: &names,
            module: key,
            parameters: BTreeSet::new(),
            found: BTreeSet::new(),
        };
        for item in &module.items {
            audit.item(item);
        }
        audit.macro_findings(execution_exports_macros::findings(&names, key, &module.items));
        found.extend(
            audit.found.into_iter().map(|message| format!("{}: {message}", module.file.display())),
        );
    }
    Ok(found.into_iter().collect())
}

/// Inspect only externally declared types, not implementation bodies or hidden state fields.
struct Audit<'a> {
    names: &'a Names,
    module: &'a [String],
    parameters: BTreeSet<String>,
    found: BTreeSet<String>,
}

impl Audit<'_> {
    /// Retain uncertainty diagnostics alongside resolved scheduler identities.
    fn resolved(&mut self, paths: BTreeSet<Vec<String>>) {
        for resolved in paths {
            if resolved.first().is_some_and(|part| part == "<type-macro>") {
                self.found.insert(format!(
                    "public type macro {} is unanalyzable; spell the execution type explicitly",
                    resolved[1]
                ));
            }
            if execution_source::restricted_path(&resolved) {
                let detail = if resolved.iter().any(|part| part == "*") {
                    "; qualify the public name instead of relying on a scheduler glob"
                } else {
                    ""
                };
                self.found
                    .insert(format!("public scheduler capability {}{detail}", resolved.join("::")));
            }
        }
    }
    /// Audit signatures of local helpers emitted by macros as part of the public boundary.
    fn macro_findings(&mut self, report: execution_exports_macros::Findings) {
        self.found.extend(report.messages);
        for helper in report.helpers {
            let mut audit = Audit {
                names: self.names,
                module: &helper[..helper.len() - 1],
                parameters: BTreeSet::new(),
                found: BTreeSet::new(),
            };
            for signature in self.names.functions(&helper) {
                audit.signature(signature);
            }
            self.found.extend(audit.found);
        }
    }
    /// Cover each Rust declaration form that can expose a raw execution capability.
    fn item(&mut self, item: &syn::Item) {
        match item {
            syn::Item::Use(item) if public(&item.vis) => self.use_tree(
                &item.tree,
                if item.leading_colon.is_some() { vec![String::new()] } else { Vec::new() },
            ),
            syn::Item::ExternCrate(item) if public(&item.vis) => {
                self.visit_path(&syn::Path::from(item.ident.clone()));
            }
            syn::Item::Type(item) if public(&item.vis) => {
                self.generics(&item.generics);
                self.visit_type(&item.ty);
                self.parameters.clear();
            }
            syn::Item::Fn(item) if public(&item.vis) => self.signature(&item.sig),
            syn::Item::Struct(item) if public(&item.vis) => {
                self.generics(&item.generics);
                for field in &item.fields {
                    if public(&field.vis) {
                        self.visit_type(&field.ty);
                    }
                }
                self.parameters.clear();
            }
            syn::Item::Union(item) if public(&item.vis) => {
                self.generics(&item.generics);
                for field in &item.fields.named {
                    if public(&field.vis) {
                        self.visit_type(&field.ty);
                    }
                }
                self.parameters.clear();
            }
            syn::Item::Enum(item) if public(&item.vis) => {
                self.generics(&item.generics);
                for variant in &item.variants {
                    for field in &variant.fields {
                        self.visit_type(&field.ty);
                    }
                }
                self.parameters.clear();
            }
            syn::Item::Trait(item) if public(&item.vis) => {
                self.generics(&item.generics);
                for bound in &item.supertraits {
                    self.visit_type_param_bound(bound);
                }
                for member in &item.items {
                    match member {
                        syn::TraitItem::Fn(member) => self.signature(&member.sig),
                        syn::TraitItem::Type(member) => {
                            let outer = self.parameters.clone();
                            self.generics(&member.generics);
                            for bound in &member.bounds {
                                self.visit_type_param_bound(bound);
                            }
                            if let Some((_, ty)) = &member.default {
                                self.visit_type(ty);
                            }
                            self.parameters = outer;
                        }
                        syn::TraitItem::Const(member) => {
                            self.visit_type(&member.ty);
                            if let Some((_, value)) = &member.default {
                                self.visit_expr(value);
                            }
                        }
                        syn::TraitItem::Macro(_) => {
                            self.found.insert("public trait macro is unanalyzable; declare execution signatures explicitly".into());
                        }
                        _ => {}
                    }
                }
                self.parameters.clear();
            }
            syn::Item::Impl(item) => self.implementation(item),
            syn::Item::Const(item) if public(&item.vis) => {
                self.visit_type(&item.ty);
                self.visit_expr(&item.expr);
            }
            syn::Item::Static(item) if public(&item.vis) => {
                self.visit_type(&item.ty);
                self.visit_expr(&item.expr);
            }
            syn::Item::ForeignMod(item) => {
                for member in &item.items {
                    match member {
                        syn::ForeignItem::Fn(member) if public(&member.vis) => {
                            self.signature(&member.sig);
                            if execution_source::restricted_path(&[member.sig.ident.to_string()]) {
                                self.found.insert(format!(
                                    "public native scheduling symbol {}",
                                    member.sig.ident
                                ));
                            }
                            for attribute in &member.attrs {
                                if let syn::Meta::NameValue(value) = &attribute.meta
                                    && value.path.is_ident("link_name")
                                    && let syn::Expr::Lit(value) = &value.value
                                    && let syn::Lit::Str(value) = &value.lit
                                    && execution_source::restricted_path(&[value.value()])
                                {
                                    self.found.insert(format!(
                                        "public native scheduling symbol {}",
                                        value.value()
                                    ));
                                }
                            }
                        }
                        syn::ForeignItem::Static(member) if public(&member.vis) => {
                            self.visit_type(&member.ty)
                        }
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }

    /// Public inherent methods and trait projections cannot reveal private runtime handles.
    fn implementation(&mut self, item: &syn::ItemImpl) {
        let parameters = generic_names(&item.generics);
        let externally_named = self.names.public_type(self.module, &item.self_ty, &parameters)
            || item.trait_.as_ref().is_some_and(|(_, path, _)| {
                self.names.public_trait(self.module, path, &parameters)
            });
        let public_member = item.items.iter().any(|member| match member {
            syn::ImplItem::Fn(member) => public(&member.vis),
            syn::ImplItem::Type(member) => public(&member.vis),
            syn::ImplItem::Const(member) => public(&member.vis),
            _ => false,
        });
        if !externally_named && !public_member {
            return;
        }
        self.generics(&item.generics);
        if externally_named {
            self.visit_type(&item.self_ty);
            if let Some((_, path, _)) = &item.trait_ {
                self.visit_path(path);
            }
        }
        for member in &item.items {
            match member {
                syn::ImplItem::Fn(member)
                    if public(&member.vis) || (externally_named && item.trait_.is_some()) =>
                {
                    self.signature(&member.sig)
                }
                syn::ImplItem::Type(member) if public(&member.vis) || externally_named => {
                    let outer = self.parameters.clone();
                    self.generics(&member.generics);
                    self.visit_type(&member.ty);
                    self.parameters = outer;
                }
                syn::ImplItem::Const(member) if public(&member.vis) || externally_named => {
                    self.visit_type(&member.ty);
                    self.visit_expr(&member.expr);
                }
                syn::ImplItem::Macro(_) if externally_named => {
                    self.found.insert("public implementation macro is unanalyzable; declare execution signatures explicitly".into());
                }
                _ => {}
            }
        }
        self.parameters.clear();
    }

    /// Retain enclosing generic parameters when a method adds its own bounds.
    fn signature(&mut self, signature: &syn::Signature) {
        let outer = self.parameters.clone();
        self.generics(&signature.generics);
        for argument in &signature.inputs {
            self.visit_fn_arg(argument);
        }
        self.visit_return_type(&signature.output);
        self.parameters = outer;
    }

    /// Generic defaults and where clauses are part of the public API.
    fn generics(&mut self, generics: &syn::Generics) {
        self.parameters.extend(generic_names(generics));
        self.visit_generics(generics);
    }

    /// Audit complete grouped re-exports, including unrestricted scheduler globs.
    fn use_tree(&mut self, tree: &syn::UseTree, mut prefix: Vec<String>) {
        match tree {
            syn::UseTree::Path(path) => {
                prefix.push(path.ident.to_string());
                self.use_tree(&path.tree, prefix);
            }
            syn::UseTree::Group(group) => {
                for item in &group.items {
                    self.use_tree(item, prefix.clone());
                }
            }
            syn::UseTree::Name(name) => {
                if name.ident != "self" {
                    prefix.push(name.ident.to_string());
                }
                self.parts(prefix);
            }
            syn::UseTree::Rename(rename) => {
                if rename.ident != "self" {
                    prefix.push(rename.ident.to_string());
                }
                self.parts(prefix);
            }
            syn::UseTree::Glob(_) => {
                if let Ok(path) = syn::parse_str::<syn::Path>(&prefix.join("::")) {
                    for mut resolved in self.names.resolve(self.module, &path, &self.parameters) {
                        resolved.push("*".into());
                        if execution_source::restricted_path(&resolved) {
                            self.found
                                .insert(format!("public scheduler glob {}", resolved.join("::")));
                        }
                    }
                }
            }
        }
    }

    /// Interpret import leaves as paths using the same module-qualified resolver as signatures.
    fn parts(&mut self, parts: Vec<String>) {
        if let Ok(path) = syn::parse_str::<syn::Path>(&parts.join("::")) {
            self.visit_path(&path);
        }
    }
}

impl<'ast> Visit<'ast> for Audit<'_> {
    /// Check nested path arguments, associated types and function-pointer signatures.
    fn visit_path(&mut self, path: &'ast syn::Path) {
        self.resolved(self.names.resolve(self.module, path, &self.parameters));
        syn::visit::visit_path(self, path);
    }

    /// Resolve normalized associated values even when the trait and self type stay private.
    fn visit_type_path(&mut self, item: &'ast syn::TypePath) {
        self.resolved(self.names.resolve_type(
            self.module,
            &syn::Type::Path(item.clone()),
            &self.parameters,
        ));
        syn::visit::visit_type_path(self, item);
    }

    /// Type macros cannot receive a clean public-boundary result without expansion.
    fn visit_type_macro(&mut self, item: &'ast syn::TypeMacro) {
        self.found.insert(format!(
            "public type macro {} is unanalyzable; spell the execution type explicitly",
            item.mac
                .path
                .segments
                .iter()
                .map(|segment| segment.ident.to_string())
                .collect::<Vec<_>>()
                .join("::")
        ));
    }

    /// Macro arguments retain literal expression paths while harmless field identifiers remain valid.
    fn visit_macro(&mut self, item: &'ast syn::Macro) {
        self.macro_findings(execution_exports_macros::invocation(self.names, self.module, item));
    }
}

/// Restricted visibility belongs to implementation ownership rather than external contracts.
fn public(visibility: &syn::Visibility) -> bool {
    matches!(visibility, syn::Visibility::Public(_))
}
