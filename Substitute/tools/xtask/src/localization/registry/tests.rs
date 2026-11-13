// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Reject malformed or parallel locale registry declarations.
use super::*;

#[test]
fn renamed_script_locale_arrays_cannot_declare_another_registry() {
    let locales = BTreeSet::from(["en".into(), "es".into()]);
    assert!(!parallel_literals("function diagnostic() { log('en'); log('es'); }", &locales));
    assert!(!parallel_literals("// const example = ['en','es']", &locales));
    assert!(parallel_literals("const renamed = { en: 'English', es: 'Español' }", &locales));
    assert!(parallel_literals("const languages = ['en','es']", &locales));
    assert!(parallel_literals("export const foreign = { first: 'en', second: 'es' }", &locales));
    assert!(!parallel_literals(
        "const fallback = 'en'; const original = 'Unicode 日本語 😀'",
        &locales
    ));
}
#[test]
fn unknown_duplicate_empty_and_noncanonical_registries_reject() -> Result<()> {
    for source in [
        "",
        "static LOCALES: &[u8] = &[];",
        "static L: &[Messages] = &[Messages{id:\"es\"}];",
        "static L: &[Messages] = &[Messages{id:\"en\"},Messages{id:\"en\"}];",
    ] {
        assert!(identities(source).is_err());
    }
    assert_eq!(
        identities("static L: &[Messages] = &[Messages{id:\"en\"},Messages{id:\"ja\"}];")?,
        BTreeSet::from(["en".into(), "ja".into()])
    );
    Ok(())
}
