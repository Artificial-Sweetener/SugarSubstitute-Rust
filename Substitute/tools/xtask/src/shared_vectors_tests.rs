// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Rejection proof for the shared-vector drift check.

use super::*;
use crate::fixtures::{Monorepo, write};

const VECTORS: &str = "Sugar/tests/x/support/vectors.json";
const LOADER: &str = "Sugar/tests/x/support/loader.py";
const PYTHON: &str = "Sugar/tests/x/test_x.py";
const RUST: &str = "crates/host/src/vectors.rs";

const GOOD_VECTORS: &str = r#"{"schema": 1, "formats": {"record": {"version": 2}}, "alpha": [{"name": "a"}, {"name": "b"}]}"#;
const GOOD_LOADER: &str = "VECTORS_PATH = Path(__file__).with_name(\"vectors.json\")\n\ndef drive(group: str) -> Mark: ...\n";
const GOOD_PYTHON: &str = "from tests.x.support.loader import load\nGROUPS = {\n    \"formats\": \"test_formats\",\n    \"alpha\": \"test_alpha\",\n}\n\n@drive(\"formats\")\ndef test_formats() -> None: ...\n\n@drive(\"alpha\")\ndef test_alpha() -> None: ...\n";
const GOOD_RUST: &str = "const V: &str = include_str!(\"../../../../Sugar/tests/x/support/vectors.json\");\nconst GROUPS: [(&str, &str); 2] = [\n    (\"formats\", \"formats\"),\n    (\"alpha\", \"alpha\"),\n];\nfn drive(name: &str) -> Vec<V> { load(name) }\n#[test]\nfn formats() { for v in drive(\"formats\") {} }\n#[test]\nfn alpha() { for v in drive(\"alpha\") {} }\n";

struct Case {
    fixture: Monorepo,
}

impl Case {
    fn new() -> Result<Self> {
        let fixture = Monorepo::new()?;
        write(fixture.monorepo(), VECTORS, GOOD_VECTORS)?;
        write(fixture.monorepo(), "Sugar/tests/x/support/copy.json", GOOD_VECTORS)?;
        write(fixture.monorepo(), LOADER, GOOD_LOADER)?;
        write(fixture.monorepo(), PYTHON, GOOD_PYTHON)?;
        write(fixture.root(), RUST, GOOD_RUST)?;
        Ok(Self { fixture })
    }

    fn sugar(&self, relative: &str, contents: &str) -> Result<()> {
        write(self.fixture.monorepo(), relative, contents)
    }

    fn rust(&self, contents: &str) -> Result<()> {
        write(self.fixture.root(), RUST, contents)
    }

    fn check(&self, catalog_version: u32) -> Result<()> {
        let components: Components = toml::from_str(&format!(
            "schema_version = 1\n[[components]]\nid = 'vectors'\nroot = 'Sugar/tests/x/support'\nowner = 'fixture'\nfiles = ['{VECTORS}']\nconsumers = ['{RUST}']\n[[shared_vectors]]\nfile = '{VECTORS}'\ncomponent = 'vectors'\nrust_suites = ['{RUST}']\npython_loader = '{LOADER}'\npython_suites = ['{PYTHON}']\nformats = {{ record = 'record-v{catalog_version}' }}\n"
        ))
        .map_err(|error| Failure::external("parse fixture components", error))?;
        let catalog: Persistence = toml::from_str(&format!(
            "schema_version = 1\n[[formats]]\nid = 'record-v{catalog_version}'\nowner = 'host'\nshared_component = 'vectors'\nsources = []\nsource_sha256 = ''\nclassification = 'authoritative_state'\npreservation = 'must_preserve'\ncurrent_version = {catalog_version}\naccepted_versions = [{catalog_version}]\ncompatibility = 'reject'\nmigrations = []\n"
        ))
        .map_err(|error| Failure::external("parse fixture catalog", error))?;
        check(self.fixture.root(), &components, &catalog)
    }
}

/// Accept two suites that read the file in place and drive every group.
#[test]
fn both_suites_driving_every_group_in_place_are_accepted() -> Result<()> {
    Case::new()?.check(2)
}

/// Reject a group the file gains until both suites drive it.
#[test]
fn an_undriven_group_is_rejected() -> Result<()> {
    let case = Case::new()?;
    case.sugar(VECTORS, r#"{"schema": 1, "formats": {"record": {"version": 2}}, "alpha": [{"name": "a"}], "beta": [{"name": "c"}]}"#)?;
    assert!(case.check(2).is_err());
    Ok(())
}

/// Reject a Rust table that leaves a group out, or names a test that does not exist.
#[test]
fn an_incomplete_rust_table_or_missing_rust_test_is_rejected() -> Result<()> {
    let case = Case::new()?;
    case.rust(&GOOD_RUST.replace("    (\"alpha\", \"alpha\"),\n", ""))?;
    assert!(case.check(2).is_err());
    case.rust(&GOOD_RUST.replace("#[test]\nfn alpha() { for v in drive(\"alpha\") {} }\n", ""))?;
    assert!(case.check(2).is_err());
    Ok(())
}

/// Reject a Python table that names a missing test, or a suite that bypasses the loader.
#[test]
fn a_missing_python_test_or_bypassed_loader_is_rejected() -> Result<()> {
    let case = Case::new()?;
    case.sugar(PYTHON, &GOOD_PYTHON.replace("def test_alpha() -> None: ...\n", ""))?;
    assert!(case.check(2).is_err());
    case.sugar(PYTHON, GOOD_PYTHON)?;
    case.check(2)?;
    case.sugar(PYTHON, &GOOD_PYTHON.replace("from tests.x.support.loader import load\n", ""))?;
    assert!(case.check(2).is_err());
    Ok(())
}

/// Reject a Rust suite that reads a copy, and a loader that opens another file.
#[test]
fn reading_a_copy_is_rejected_in_either_suite() -> Result<()> {
    let case = Case::new()?;
    case.rust(&GOOD_RUST.replace("support/vectors.json", "support/copy.json"))?;
    assert!(case.check(2).is_err());
    let case = Case::new()?;
    case.sugar(LOADER, "VECTORS_PATH = Path(__file__).with_name(\"copy.json\")\n")?;
    assert!(case.check(2).is_err());
    Ok(())
}

/// Reject a declared format version that differs from the catalog's current version.
#[test]
fn a_format_version_out_of_step_with_the_catalog_is_rejected() -> Result<()> {
    assert!(Case::new()?.check(3).is_err());
    Ok(())
}

/// Reject duplicate or unnamed vectors in a group.
#[test]
fn duplicate_vector_names_are_rejected() -> Result<()> {
    let case = Case::new()?;
    case.sugar(VECTORS, r#"{"schema": 1, "formats": {"record": {"version": 2}}, "alpha": [{"name": "a"}, {"name": "a"}]}"#)?;
    assert!(case.check(2).is_err());
    case.sugar(
        VECTORS,
        r#"{"schema": 1, "formats": {"record": {"version": 2}}, "alpha": [{"text": "unnamed"}]}"#,
    )?;
    assert!(case.check(2).is_err());
    Ok(())
}

/// Reject a driver that does not take its own group through the accessor in either suite, or a
/// suite that does not define the accessor, since then a vector could go unexercised.
#[test]
fn a_driver_bypassing_the_accessor_is_rejected() -> Result<()> {
    let case = Case::new()?;
    case.rust(
        &GOOD_RUST.replace("for v in drive(\"alpha\") {}", "for v in drive(\"formats\") {}"),
    )?;
    assert!(case.check(2).is_err());
    case.rust(&GOOD_RUST.replace("for v in drive(\"alpha\") {}", "let v = load(\"alpha\")[0];"))?;
    assert!(case.check(2).is_err());
    case.rust(&GOOD_RUST.replace("fn drive(name: &str)", "fn all(name: &str)"))?;
    assert!(case.check(2).is_err());
    case.rust(GOOD_RUST)?;
    case.check(2)?;
    case.sugar(PYTHON, &GOOD_PYTHON.replace("@drive(\"alpha\")\n", ""))?;
    assert!(case.check(2).is_err());
    case.sugar(PYTHON, &GOOD_PYTHON.replace("@drive(\"alpha\")\n", "@drive(\"formats\")\n"))?;
    assert!(case.check(2).is_err());
    case.sugar(PYTHON, GOOD_PYTHON)?;
    case.sugar(LOADER, "VECTORS_PATH = Path(__file__).with_name(\"vectors.json\")\n")?;
    assert!(case.check(2).is_err());
    Ok(())
}
