// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Inspect Rust capability access through imports, aliases, macros and qualified references.
use crate::error::{Failure, Result};
use proc_macro2::{TokenStream, TokenTree};
use std::collections::{BTreeMap, BTreeSet};
use syn::visit::Visit;

/// Retain import aliases and original import paths before inspecting executable token trees.
#[derive(Default)]
struct Imports {
    aliases: BTreeMap<String, BTreeSet<Vec<String>>>,
    paths: Vec<Vec<String>>,
    macros: Vec<(String, BTreeSet<String>)>,
    capture_literals: bool,
    literals: Vec<String>,
}

impl<'ast> Visit<'ast> for Imports {
    /// Decode Rust string/byte literals only for owners which inspect embedded capabilities.
    fn visit_expr_lit(&mut self, expression: &'ast syn::ExprLit) {
        if self.capture_literals {
            literal_value(&expression.lit, &mut self.literals);
        }
        syn::visit::visit_expr_lit(self, expression);
    }

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
        if self.capture_literals {
            collect_literals(item.tokens.clone(), &mut self.literals);
        }
        let mut identifiers = BTreeSet::new();
        macro_identifiers(item.tokens.clone(), &mut identifiers);
        self.macros.push((
            item.path.segments.last().map(|segment| segment.ident.to_string()).unwrap_or_default(),
            identifiers,
        ));
    }

    /// Retain type aliases that can redirect a contract constructor or producer trait.
    fn visit_item_type(&mut self, item: &'ast syn::ItemType) {
        if let syn::Type::Path(ty) = item.ty.as_ref() {
            let path = ty.path.segments.iter().map(|part| part.ident.to_string()).collect();
            self.aliases.entry(item.ident.to_string()).or_default().insert(path);
        }
        syn::visit::visit_item_type(self, item);
    }

    /// Classify renamed foreign functions through their explicit linker identities.
    fn visit_attribute(&mut self, attribute: &'ast syn::Attribute) {
        if let syn::Meta::NameValue(value) = &attribute.meta
            && value.path.is_ident("link_name")
            && let syn::Expr::Lit(literal) = &value.value
            && let syn::Lit::Str(name) = &literal.lit
        {
            self.paths.push(vec!["link_name".into(), name.value()]);
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
                // A grouped `self` import binds the enclosing module's actual name.
                let binding = prefix.last().cloned().unwrap_or_else(|| name.ident.to_string());
                self.aliases.entry(binding).or_default().insert(prefix);
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

/// Preserve decoded literals inside macro arguments and transcribers without expanding code.
fn collect_literals(tokens: TokenStream, output: &mut Vec<String>) {
    for token in tokens {
        match token {
            TokenTree::Group(group) => collect_literals(group.stream(), output),
            TokenTree::Literal(literal) => {
                if let Ok(literal) = syn::parse_str::<syn::Lit>(&literal.to_string()) {
                    literal_value(&literal, output);
                }
            }
            _ => {}
        }
    }
}
/// Attribute strings are interpreted separately by the linker-name contract.
fn literal_value(literal: &syn::Lit, output: &mut Vec<String>) {
    match literal {
        syn::Lit::Str(value) => output.push(value.value()),
        syn::Lit::CStr(value) => {
            if let Ok(value) = value.value().into_string() {
                output.push(value);
            }
        }
        syn::Lit::ByteStr(value) => {
            if let Ok(value) = String::from_utf8(value.value()) {
                output.push(value);
            }
        }
        _ => {}
    }
}

/// Inspect macro identifiers across groups without treating string arguments as executable paths.
pub(crate) fn macro_identifiers(tokens: TokenStream, identifiers: &mut BTreeSet<String>) {
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

/// Scan groups recursively, including macro definitions and arguments, while ignoring literals.
fn inspect(
    tokens: TokenStream,
    prefix: &[String],
    imports: &Imports,
    found: &mut BTreeSet<String>,
    restricted: fn(&[String]) -> bool,
    ignore_bare: fn(&str) -> bool,
) {
    let tokens: Vec<_> = tokens.into_iter().collect();
    let mut index = 0;
    while index < tokens.len() {
        match &tokens[index] {
            TokenTree::Group(group) => {
                inspect(group.stream(), prefix, imports, found, restricted, ignore_bare)
            }
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
                            inspect(group.stream(), &path, imports, found, restricted, ignore_bare);
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
                        && expanded.first().is_some_and(|name| ignore_bare(name));
                    if !ordinary_name && restricted(&expanded) {
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

/// Resolve selected public signature paths through the same import and type-alias inventory.
pub(crate) fn resolved_paths_match(
    text: &str,
    paths: &[Vec<String>],
    restricted: impl Fn(&[String]) -> bool,
) -> Result<bool> {
    let file = syn::parse_file(text)
        .map_err(|cause| Failure::external("parse selected capability paths", cause))?;
    let mut imports = Imports::default();
    imports.visit_file(&file);
    Ok(paths
        .iter()
        .any(|path| imports.expand(path.clone()).iter().any(|resolved| restricted(resolved))))
}

/// Return deterministic findings across all syntax, including inactive targets and macro tokens.
pub(crate) fn findings(
    source: &str,
    restricted: fn(&[String]) -> bool,
    fragments: fn(&BTreeSet<String>) -> bool,
    ignore_bare: fn(&str) -> bool,
) -> Result<Vec<String>> {
    findings_with_literals(source, restricted, fragments, ignore_bare, None, &[])
}
/// Additionally classify decoded embedded script/native-symbol literals under an explicit owner.
pub(crate) fn findings_with_literals(
    source: &str,
    restricted: fn(&[String]) -> bool,
    fragments: fn(&BTreeSet<String>) -> bool,
    ignore_bare: fn(&str) -> bool,
    literal: Option<fn(&str) -> bool>,
    external_bindings: &[(String, String)],
) -> Result<Vec<String>> {
    let parsed = syn::parse_file(source)
        .map_err(|error| Failure::external("parse capability access", error))?;
    let mut imports = Imports { capture_literals: literal.is_some(), ..Imports::default() };
    for (binding, identity) in external_bindings {
        imports.aliases.entry(binding.clone()).or_default().insert(vec![identity.clone()]);
    }
    imports.visit_file(&parsed);
    let mut found = BTreeSet::new();
    if let Some(literal) = literal
        && imports.literals.iter().any(|value| literal(value))
    {
        found.insert("literal restricted capability".into());
    }
    for (name, identifiers) in &imports.macros {
        let expanded: BTreeSet<_> = identifiers
            .iter()
            .flat_map(|name| imports.expand(vec![name.clone()]))
            .flatten()
            .collect();
        if fragments(&expanded) {
            found.insert(format!("{name}! restricted capability fragments"));
        }
    }
    for path in &imports.paths {
        for expanded in imports.expand(path.clone()) {
            if restricted(&expanded) {
                found.insert(expanded.join("::"));
            }
        }
    }
    let tokens =
        source.parse().map_err(|error| Failure::external("tokenize capability access", error))?;
    inspect(tokens, &[], &imports, &mut found, restricted, ignore_bare);
    Ok(found.into_iter().collect())
}
