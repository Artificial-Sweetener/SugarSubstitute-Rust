// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Track local literal provenance into known product records, titles and locale registries.
use crate::error::{Failure, Result};
use std::collections::{BTreeMap, BTreeSet};
use syn::{Expr, visit::Visit};

/// Seed imported and type aliases, then solve local literal/wrapper provenance to a fixed point.
#[derive(Default)]
struct Copy {
    aliases: BTreeMap<String, String>,
    definitions: BTreeMap<String, Expr>,
    authored: BTreeSet<String>,
    findings: BTreeSet<String>,
    locales: BTreeSet<String>,
}
impl Copy {
    /// Follow aliases conservatively across lexical scopes; conflicting definitions cannot erase evidence.
    fn name(&self, path: &syn::Path) -> String {
        let mut name = path.segments.last().map(|part| part.ident.to_string()).unwrap_or_default();
        let mut seen = BTreeSet::new();
        while seen.insert(name.clone()) {
            let Some(next) = self.aliases.get(&name) else {
                break;
            };
            name = next.clone();
        }
        name
    }
    /// Propagate any nonempty string literal, including formatting and string-building wrappers.
    fn literal(&self, expression: &Expr) -> bool {
        match expression {
            Expr::Lit(value) => {
                matches!(&value.lit, syn::Lit::Str(text) if !text.value().is_empty())
            }
            Expr::Path(value) => self.authored.contains(&self.name(&value.path)),
            Expr::Reference(value) => self.literal(&value.expr),
            Expr::Paren(value) => self.literal(&value.expr),
            Expr::Binary(value) => self.literal(&value.left) || self.literal(&value.right),
            Expr::Call(value) => {
                self.literal(&value.func) || value.args.iter().any(|arg| self.literal(arg))
            }
            Expr::MethodCall(value) => {
                self.literal(&value.receiver) || value.args.iter().any(|arg| self.literal(arg))
            }
            Expr::Macro(value) => has_text(value.mac.tokens.clone()),
            Expr::Block(value) => value.block.stmts.iter().any(
                |statement| matches!(statement,syn::Stmt::Expr(value,_) if self.literal(value)),
            ),
            _ => false,
        }
    }
}
impl<'ast> Visit<'ast> for Copy {
    fn visit_item_use(&mut self, item: &'ast syn::ItemUse) {
        fn collect(tree: &syn::UseTree, aliases: &mut BTreeMap<String, String>) {
            match tree {
                syn::UseTree::Path(path) => collect(&path.tree, aliases),
                syn::UseTree::Group(group) => {
                    for tree in &group.items {
                        collect(tree, aliases);
                    }
                }
                syn::UseTree::Rename(rename) => {
                    aliases.insert(rename.rename.to_string(), rename.ident.to_string());
                }
                _ => {}
            }
        }
        collect(&item.tree, &mut self.aliases);
    }
    fn visit_item_type(&mut self, item: &'ast syn::ItemType) {
        if let syn::Type::Path(path) = item.ty.as_ref() {
            self.aliases.insert(item.ident.to_string(), self.name(&path.path));
        }
        syn::visit::visit_item_type(self, item);
    }
    fn visit_local(&mut self, item: &'ast syn::Local) {
        if let (syn::Pat::Ident(name), Some(initializer)) = (&item.pat, &item.init) {
            if self.literal(&initializer.expr) {
                self.authored.insert(name.ident.to_string());
            }
            self.definitions.insert(name.ident.to_string(), *initializer.expr.clone());
        }
        syn::visit::visit_local(self, item);
    }
    fn visit_item_const(&mut self, item: &'ast syn::ItemConst) {
        if matches!(item.ident.to_string().as_str(), "LOCALES" | "SUPPORTED_LOCALES") {
            self.findings.insert("duplicate locale registry".into());
        }
        self.definitions.insert(item.ident.to_string(), *item.expr.clone());
        if self.literal(&item.expr) {
            self.authored.insert(item.ident.to_string());
        }
        syn::visit::visit_item_const(self, item);
    }
    fn visit_item_fn(&mut self, item: &'ast syn::ItemFn) {
        self.definitions.insert(
            item.sig.ident.to_string(),
            Expr::Block(syn::ExprBlock { attrs: vec![], label: None, block: *item.block.clone() }),
        );
        syn::visit::visit_item_fn(self, item);
    }
    fn visit_expr_struct(&mut self, item: &'ast syn::ExprStruct) {
        let name = self.name(&item.path);
        if matches!(
            name.as_str(),
            "Messages"
                | "InstallerMessages"
                | "ReportMessages"
                | "WindowMessages"
                | "StartupMessages"
                | "ConflictMessages"
        ) {
            self.findings.insert(format!("catalog construction {name}"));
        }
        if matches!(
            name.as_str(),
            "AccessibleControl"
                | "MainCaptionLabels"
                | "MainRecoveryLabels"
                | "DesktopOptions"
                | "InstallerWindow"
        ) {
            for field in &item.fields {
                if let syn::Member::Named(member) = &field.member
                    && matches!(
                        member.to_string().as_str(),
                        "title"
                            | "label"
                            | "retry"
                            | "retry_ready"
                            | "minimize"
                            | "maximize"
                            | "restore"
                            | "close"
                    )
                    && self.literal(&field.expr)
                {
                    self.findings.insert(format!("owned literal {name}.{member}"));
                }
            }
        }
        syn::visit::visit_expr_struct(self, item);
    }
    fn visit_expr_method_call(&mut self, item: &'ast syn::ExprMethodCall) {
        if matches!(item.method.to_string().as_str(), "with_title" | "set_title")
            && item.args.iter().any(|arg| self.literal(arg))
        {
            self.findings.insert(format!("owned literal {}", item.method));
        }
        syn::visit::visit_expr_method_call(self, item);
    }
    fn visit_item_static(&mut self, item: &'ast syn::ItemStatic) {
        if matches!(item.ident.to_string().as_str(), "LOCALES" | "SUPPORTED_LOCALES") {
            self.findings.insert("duplicate locale registry".into());
        }
        syn::visit::visit_item_static(self, item);
    }
    fn visit_macro(&mut self, item: &'ast syn::Macro) {
        let mut names = BTreeSet::new();
        let mut text = BTreeSet::new();
        macro_values(item.tokens.clone(), &mut names, &mut text);
        let names: BTreeSet<_> = names
            .into_iter()
            .map(|name| {
                syn::parse_str::<syn::Path>(&name).map_or(name.clone(), |path| self.name(&path))
            })
            .collect();
        if names.iter().any(|name| {
            matches!(
                name.as_str(),
                "Messages"
                    | "InstallerMessages"
                    | "ReportMessages"
                    | "WindowMessages"
                    | "StartupMessages"
                    | "ConflictMessages"
            )
        }) {
            self.findings.insert("macro catalog construction".into());
        }
        if !text.is_empty()
            && names.iter().any(|name| {
                matches!(
                    name.as_str(),
                    "AccessibleControl"
                        | "MainCaptionLabels"
                        | "MainRecoveryLabels"
                        | "DesktopOptions"
                        | "InstallerWindow"
                )
            })
        {
            self.findings.insert("macro owned presentation literal".into());
        }
        if text.intersection(&self.locales).count() > 1 {
            self.findings.insert("macro parallel locale inventory".into());
        }
        syn::visit::visit_macro(self, item);
    }
    fn visit_expr_array(&mut self, item: &'ast syn::ExprArray) {
        let locales = item
            .elems
            .iter()
            .filter_map(|value| match value {
                Expr::Lit(value) => match &value.lit {
                    syn::Lit::Str(value) => Some(value.value()),
                    _ => None,
                },
                _ => None,
            })
            .filter(|value| self.locales.contains(value))
            .collect::<BTreeSet<_>>();
        if locales.len() > 1 {
            self.findings.insert("parallel locale inventory".into());
        }
        syn::visit::visit_expr_array(self, item);
    }
}
/// Formatting/concatenation macros cannot conceal authored strings in product positions.
fn has_text(tokens: proc_macro2::TokenStream) -> bool {
    tokens.into_iter().any(|token| match token {
        proc_macro2::TokenTree::Literal(value) => syn::parse_str::<syn::LitStr>(&value.to_string())
            .is_ok_and(|text| !text.value().is_empty()),
        proc_macro2::TokenTree::Group(value) => has_text(value.stream()),
        _ => false,
    })
}
/// Analyze all syntax without evaluating cfg, including tests and executable targets.
pub(super) fn findings(source: &str, locales: &BTreeSet<String>) -> Result<Vec<String>> {
    let file =
        syn::parse_file(source).map_err(|cause| Failure::external("parse owned text", cause))?;
    let mut copy = Copy { locales: locales.clone(), ..Copy::default() };
    copy.visit_file(&file);
    loop {
        let added: Vec<_> = copy
            .definitions
            .iter()
            .filter(|(name, value)| !copy.authored.contains(*name) && copy.literal(value))
            .map(|(name, _)| name.clone())
            .collect();
        if added.is_empty() {
            break;
        }
        copy.authored.extend(added);
    }
    copy.findings.clear();
    copy.visit_file(&file);
    Ok(copy.findings.into_iter().collect())
}

/// Preserve macro argument/declaration identities and literal text before cfg or expansion.
fn macro_values(
    tokens: proc_macro2::TokenStream,
    names: &mut BTreeSet<String>,
    text: &mut BTreeSet<String>,
) {
    for token in tokens {
        match token {
            proc_macro2::TokenTree::Ident(value) => {
                names.insert(value.to_string());
            }
            proc_macro2::TokenTree::Literal(value) => {
                if let Ok(value) = syn::parse_str::<syn::LitStr>(&value.to_string()) {
                    text.insert(value.value());
                }
            }
            proc_macro2::TokenTree::Group(value) => macro_values(value.stream(), names, text),
            _ => {}
        }
    }
}
