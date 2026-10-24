// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Adversarial capability, copy-provenance, registry and coverage rejection fixtures.
use super::*;
use crate::fixtures::{self, Repository};

fn fixture() -> Result<(Repository, Policy)> {
    let mut repository = Repository::new()?;
    repository.architecture.owners[0].layer = "domain".into();
    repository.architecture.owners[0].auxiliary_roots.push("worker".into());
    for (path, source) in [
        (
            "crates/host/src/localization.rs",
            "pub static LOCALES: &[Messages] = &[Messages{id:\"en\"},Messages{id:\"es\"}];",
        ),
        ("crates/host/src/localization/en.rs", "const COPY: &str = \"Loading\";"),
        ("worker/main.ts", "export const ready = true"),
        ("installer/package.json", "{}"),
        ("native/bridge.cc", "int initialized = 1;"),
        ("installer/tests/fixture.js", "export const ready = true"),
    ] {
        fixtures::write(repository.root(), path, source)?;
    }
    let policy = Policy {
        schema_version: 1,
        owner: "host".into(),
        registry: "crates/host/src/localization.rs".into(),
        catalog_root: "crates/host/src/localization".into(),
        auxiliary_roots: vec!["native".into(), "installer/tests".into()],
        script_manifests: vec!["installer/package.json".into()],
        records: vec![],
    };
    Ok((repository, policy))
}
fn check(repository: &Repository, policy: &Policy) -> Result<()> {
    super::check(repository.root(), &repository.architecture, &repository.workspace, policy)
}

#[test]
fn aliases_reexports_wrappers_macros_foreign_names_and_inactive_targets_require_review()
-> Result<()> {
    for source in [
        "use windows::Win32::UI::WindowsAndMessaging::MessageBoxW as ordinary;",
        "pub use windows::Win32::UI::WindowsAndMessaging::*;",
        "fn wrapper() { native::MessageBoxW(a,b,c,d); }",
        "macro_rules! caption { ($root:ident) => { $root::MessageBoxW(a,b,c,d) } }",
        "macro_rules! call { ($root:ident,$sink:ident) => { $root::$sink(a,b,c,d) } } fn f(){call!(native, MessageBoxW);}",
        "#[cfg(unix)] fn other(){view.set_title(title);}",
        "#[cfg(test)] mod tests { fn fixture(){view.with_title(title);} }",
        "unsafe extern \"system\" { #[link_name = \"MessageBoxW\"] fn innocent(); }",
        "use ui::InstallerWindow as Surface; fn f(){Surface::new(title);}",
        "fn f(){ui::DesktopOptions::new(text);}",
        "fn f(){label.set_text(text);}",
        "fn f(){label.set_label(text);}",
        "fn f(){label.set_markup(text);}",
    ] {
        let (repository, policy) = fixture()?;
        fixtures::write(repository.root(), "crates/host/src/bin/bypass.rs", source)?;
        assert!(check(&repository, &policy).is_err(), "accepted {source}");
    }
    Ok(())
}

#[test]
fn local_literal_provenance_rejects_constants_wrappers_aliases_and_formatting() -> Result<()> {
    for source in [
        "fn f(){let label = \"English\"; let control = AccessibleControl { label };}",
        "fn text()-> &'static str { \"English\" } fn f(){let c=AccessibleControl{label:text()};}",
        "const TEXT: &str = \"English\"; fn f(){let a=TEXT; let b=a; let c=AccessibleControl{label:b};}",
        "use ui::AccessibleControl as C; fn f(){let c=C{label:format!(\"English {}\",n)};}",
        "type C = ui::MainCaptionLabels; fn f(){let c=C{close:concat!(\"Cl\",\"ose\")};}",
        "fn f(){let a=\"English\"; window.with_title(a);}",
        "fn f(){ let c=localization::Messages { id: \"fr\" }; }",
        "pub static LOCALES: &[&str] = &[\"fr\"];",
        "pub const LOCALES: &[&str] = &[\"fr\"];",
        "use copy::Messages as Text; macro_rules! bypass { () => {Text {id:\"fr\"}} }",
        "macro_rules! bypass { () => {AccessibleControl {label:concat!(\"Eng\",\"lish\")}} }",
        "fn f(){let available = vec![\"en\",\"es\"];} ",
        "const CHOICES: &[&str] = &[\"en\",\"es\"];",
    ] {
        assert!(
            !rust_copy::findings(source, &BTreeSet::from(["en".into(), "es".into()]))?.is_empty(),
            "missed {source}"
        );
        let (repository, mut policy) = fixture()?;
        let path = "crates/host/src/bypass.rs";
        fixtures::write(repository.root(), path, source)?;
        policy.records.push(Record {
            path: path.into(),
            source_sha256: fingerprint(repository.root(), &[path.into()])?,
            kind: Kind::Presentation,
            rationale: "Cannot authorize owned literal bypass".into(),
        });
        assert!(check(&repository, &policy).is_err(), "accepted {source}");
    }
    Ok(())
}

#[test]
fn every_authored_language_and_auxiliary_root_rejects_computed_or_aliased_sinks() -> Result<()> {
    for extension in EXTENSIONS.iter().filter(|extension| **extension != "rs") {
        for source in [
            "element['text' + 'Content'] = ownedText",
            "const method = 'innerText'; element[method] = ownedText",
            "import { alert as ordinary } from 'presentation'; ordinary(text)",
            "const setter = element.setAttribute.bind(element); setter(name,text)",
            "MessageBoxW(0, text, title, 0)",
            "const supportedLocales = ['en','es']",
            "const setter = Reflect.set; setter(element, key, text)",
            "Object.defineProperty(element, key, {value:text})",
            "Object['define' + 'Property'](element, key, {value:text})",
            "element.append(text)",
            "element.replaceChildren(text)",
            "new Option(text, value)",
            "Function(source)()",
        ] {
            let (repository, policy) = fixture()?;
            fixtures::write(repository.root(), &format!("worker/bypass.{extension}"), source)?;
            assert!(check(&repository, &policy).is_err(), "accepted {extension}: {source}");
        }
    }
    for path in [
        "native/extra.mm",
        "installer/tests/extra.js",
        "crates/host/examples/extra.rs",
        "worker/extra.Js",
        "worker/extra.SvG",
        "crates/host/src/extra.Rs",
    ] {
        let (repository, policy) = fixture()?;
        fixtures::write(
            repository.root(),
            path,
            if path.to_ascii_lowercase().ends_with("rs") {
                "fn f(){native::MessageBoxW(a,b,c,d);}"
            } else {
                "MessageBoxW(a,b,c,d)"
            },
        )?;
        assert!(check(&repository, &policy).is_err());
    }
    Ok(())
}

#[test]
fn diagnostics_protocol_literals_user_text_and_localized_fields_are_distinct() -> Result<()> {
    assert!(rust_copy::findings("fn f(){let diagnostic=\"Error opening file\"; let protocol=\"ready\"; let c=AccessibleControl{label:copy.cancel};}", &BTreeSet::new())?.is_empty());
    assert!(
        lexical_findings(
            "const diagnostic = 'connection failed'; send('diagnostic', {message:cause.message})"
        )
        .is_empty()
    );
    let (repository, policy) = fixture()?;
    check(&repository, &policy)
}

#[test]
fn schema_stale_unused_duplicate_foreign_owner_and_missing_coverage_reject() -> Result<()> {
    let (repository, mut policy) = fixture()?;
    policy.schema_version = 2;
    assert!(check(&repository, &policy).is_err());
    policy.schema_version = 1;
    policy.auxiliary_roots.pop();
    assert!(check(&repository, &policy).is_err());
    let (repository, mut policy) = fixture()?;
    policy.script_manifests.clear();
    assert!(check(&repository, &policy).is_err());
    let (repository, mut policy) = fixture()?;
    policy.owner = "foreign".into();
    assert!(check(&repository, &policy).is_err());
    let (repository, mut policy) = fixture()?;
    let path = "worker/main.ts";
    fixtures::write(repository.root(), path, "element.textContent = bootstrap.messages.title")?;
    policy.records.push(Record {
        path: path.into(),
        source_sha256: fingerprint(repository.root(), &[path.into()])?,
        kind: Kind::Presentation,
        rationale: "Host-localized copy".into(),
    });
    check(&repository, &policy)?;
    policy.records.push(Record {
        path: path.into(),
        source_sha256: fingerprint(repository.root(), &[path.into()])?,
        kind: Kind::Presentation,
        rationale: "Duplicate review must reject".into(),
    });
    assert!(check(&repository, &policy).is_err());
    policy.records.pop();
    fixtures::write(repository.root(), path, "element.textContent = 'new English'")?;
    assert!(check(&repository, &policy).is_err());
    fixtures::write(repository.root(), path, "export const ready=true")?;
    policy.records[0].source_sha256 = fingerprint(repository.root(), &[path.into()])?;
    assert!(check(&repository, &policy).is_err());
    let complete = "schema_version=1\nowner='host'\nregistry='crates/host/src/localization.rs'\ncatalog_root='crates/host/src/localization'\nauxiliary_roots=['native','installer/tests']\nscript_manifests=['installer/package.json']\nrecords=[]\n";
    assert!(toml::from_str::<Policy>(complete).is_ok());
    assert!(toml::from_str::<Policy>(&format!("{complete}unknown=true")).is_err());
    Ok(())
}

#[test]
fn renamed_dependency_identity_cannot_install_an_independent_localizer() -> Result<()> {
    let (mut repository, policy) = fixture()?;
    repository.workspace.packages[0].dependencies.push(
        serde_json::from_value(
            serde_json::json!({"name":"fluent-bundle","rename":"ordinary","path":null,"kind":null}),
        )
        .map_err(|cause| Failure::external("dependency fixture", cause))?,
    );
    assert!(check(&repository, &policy).is_err());
    Ok(())
}
