// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Inspect the actual closed namespace registry and cache-root constant without a second inventory.
use super::*;

/// Require exhaustive unit variants, explicit directory arms and complete initialization inventory.
pub(super) fn names(source: &str) -> Result<BTreeSet<String>> {
    let syntax = syn::parse_file(source)
        .map_err(|cause| Failure::external("parse cache registry", cause))?;
    let mut variants = BTreeSet::new();
    let mut initialized = None;
    let mut mappings = None;
    let mut enums = 0;
    for item in syntax.items {
        match item {
            syn::Item::Enum(item) if item.ident == "Namespace" => {
                enums += 1;
                for entry in item.variants {
                    require(
                        matches!(entry.fields, syn::Fields::Unit)
                            && variants.insert(entry.ident.to_string()),
                        "cache namespaces require unique unit variants",
                    )?;
                }
            }
            syn::Item::Impl(item)
                if item.trait_.is_none()
                    && matches!(item.self_ty.as_ref(), syn::Type::Path(owner) if owner.path.is_ident("Namespace")) =>
            {
                for member in item.items {
                    match member {
                        syn::ImplItem::Const(value) if value.ident == "ALL" => {
                            require(
                                initialized.is_none(),
                                "cache initialization registry must be unambiguous",
                            )?;
                            let syn::Expr::Array(array) = value.expr else {
                                return Err(Failure::Policy(
                                    "cache ALL must be an explicit variant array".into(),
                                ));
                            };
                            let mut values = BTreeSet::new();
                            for element in array.elems {
                                let syn::Expr::Path(path) = element else {
                                    return Err(Failure::Policy(
                                        "cache ALL must contain explicit variants".into(),
                                    ));
                                };
                                require(
                                    values.insert(variant(&path.path)?),
                                    "cache initialization variants must be unique",
                                )?;
                            }
                            initialized = Some(values);
                        }
                        syn::ImplItem::Fn(value) if value.sig.ident == "directory" => {
                            require(
                                mappings.is_none(),
                                "cache directory registry must be unambiguous",
                            )?;
                            require(
                                value.block.stmts.len() == 1,
                                "cache directories require one explicit match",
                            )?;
                            let Some(syn::Stmt::Expr(syn::Expr::Match(mapping), _)) =
                                value.block.stmts.first()
                            else {
                                return Err(Failure::Policy(
                                    "cache directories require an explicit match".into(),
                                ));
                            };
                            let mut entries = std::collections::BTreeMap::new();
                            for arm in &mapping.arms {
                                require(
                                    arm.guard.is_none(),
                                    "cache directory arms cannot be conditional",
                                )?;
                                let syn::Pat::Path(pattern) = &arm.pat else {
                                    return Err(Failure::Policy(
                                        "cache directories require explicit variants".into(),
                                    ));
                                };
                                let name = variant(&pattern.path)?;
                                let syn::Expr::Lit(value) = arm.body.as_ref() else {
                                    return Err(Failure::Policy(
                                        "cache directories require literal names".into(),
                                    ));
                                };
                                let syn::Lit::Str(value) = &value.lit else {
                                    return Err(Failure::Policy(
                                        "cache directories require string names".into(),
                                    ));
                                };
                                require(
                                    entries.insert(name, value.value()).is_none(),
                                    "cache directory variants must be unique",
                                )?;
                            }
                            mappings = Some(entries);
                        }
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }
    require(
        enums == 1 && !variants.is_empty(),
        "cache registry requires one nonempty Namespace enum",
    )?;
    require(
        initialized.as_ref() == Some(&variants),
        "cache initialization must include every namespace exactly once",
    )?;
    let mappings =
        mappings.ok_or_else(|| Failure::Policy("cache directory mapping is absent".into()))?;
    require(
        mappings.keys().cloned().collect::<BTreeSet<_>>() == variants,
        "cache directory mapping must cover every namespace exactly once",
    )?;
    let values: Vec<_> = mappings.values().map(String::as_str).collect();
    catalog::check_namespaces(&values)?;
    Ok(mappings.into_values().collect())
}
/// Read the actual schema-qualified scope directory from the storage owner's entry module.
pub(super) fn directory(source: &str) -> Result<String> {
    let syntax = syn::parse_file(source)
        .map_err(|cause| Failure::external("parse cache entry source", cause))?;
    let mut directory = None;
    for item in syntax.items {
        if let syn::Item::Const(item) = item
            && item.ident == "DIRECTORY"
        {
            require(directory.is_none(), "cache scope directory must be unambiguous")?;
            let syn::Expr::Lit(value) = *item.expr else {
                return Err(Failure::Policy(
                    "cache scope directory must be an explicit string".into(),
                ));
            };
            let syn::Lit::Str(value) = value.lit else {
                return Err(Failure::Policy("cache scope directory must be a string".into()));
            };
            directory = Some(value.value());
        }
    }
    let directory =
        directory.ok_or_else(|| Failure::Policy("cache scope directory is absent".into()))?;
    catalog::check_namespaces(&[&directory])?;
    Ok(directory)
}
/// Reject wildcard, alias and computed variants in the authoritative registry.
fn variant(path: &syn::Path) -> Result<String> {
    require(
        path.leading_colon.is_none()
            && path.segments.len() == 2
            && path.segments.first().is_some_and(|segment| segment.ident == "Self")
            && path
                .segments
                .iter()
                .all(|segment| matches!(segment.arguments, syn::PathArguments::None)),
        "cache registry variants must use Self::Variant",
    )?;
    path.segments
        .last()
        .map(|segment| segment.ident.to_string())
        .ok_or_else(|| Failure::Policy("cache registry variant is absent".into()))
}

#[cfg(test)]
mod tests {
    use super::*;
    const VALID: &str = r#"enum Namespace { A, B }
impl Namespace {
    const ALL: [Self; 2] = [Self::A, Self::B];
    fn directory(self) -> &'static str { match self { Self::A => "a-v1", Self::B => "b-v1" } }
}"#;

    #[test]
    fn registry_rejects_missing_initialization_implicit_mapping_aliases_and_unknown_variants()
    -> Result<()> {
        assert_eq!(names(VALID)?, BTreeSet::from(["a-v1".into(), "b-v1".into()]));
        for source in [
            String::new(),
            VALID.replace("enum Namespace { A, B }", "enum Namespace { A, B, C }"),
            VALID.replace("[Self::A, Self::B]", "[Self::A, Self::A]"),
            VALID.replace("const ALL: [Self; 2] = [Self::A, Self::B];", ""),
            VALID.replace("Self::B =>", "_ =>"),
            VALID.replace("Self::B =>", "Alias::B =>"),
            VALID.replace("\"b-v1\"", "computed()"),
            VALID.replace("\"b-v1\"", "\"a-v1\""),
            VALID.replace("\"b-v1\"", "\"../escape\""),
        ] {
            assert!(names(&source).is_err(), "accepted {source}");
        }
        Ok(())
    }
    #[test]
    fn scope_directory_rejects_unknown_computed_duplicate_and_invalid_paths() -> Result<()> {
        assert_eq!(
            directory("pub const DIRECTORY: &str = \"runtime-cache-v1\";")?,
            "runtime-cache-v1"
        );
        for source in [
            "",
            "const DIRECTORY: &str = compute();",
            "const DIRECTORY: &str = \"../cache\";",
            "const DIRECTORY: &str = \"a\"; const DIRECTORY: &str = \"b\";",
        ] {
            assert!(directory(source).is_err());
        }
        Ok(())
    }
}
