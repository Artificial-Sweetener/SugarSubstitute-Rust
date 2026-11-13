// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Prove the fingerprint task records exactly what the gates compute, and nothing else.

use super::*;
use crate::source::fingerprint;

/// A monorepo with a `workspace` folder holding `governance/` and authored sources.
struct Monorepo {
    _directory: tempfile::TempDir,
    workspace: PathBuf,
}

impl Monorepo {
    fn new() -> Result<Self> {
        let directory =
            tempfile::tempdir().map_err(|error| Failure::external("create fixture", error))?;
        let workspace = directory.path().join("workspace");
        std::fs::create_dir_all(workspace.join("governance/nested"))
            .map_err(|error| Failure::external("create fixture governance", error))?;
        Ok(Self { _directory: directory, workspace })
    }

    fn write(&self, relative: &str, text: &str) -> Result<()> {
        let path = self.workspace.join(relative);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|error| Failure::external("create fixture folder", error))?;
        }
        std::fs::write(path, text).map_err(|error| Failure::external("write fixture", error))
    }

    fn read(&self, relative: &str) -> Result<String> {
        std::fs::read_to_string(self.workspace.join(relative))
            .map_err(|error| Failure::external("read fixture", error))
    }

    fn hash(&self, sources: &[&str]) -> Result<String> {
        fingerprint(
            &self.workspace,
            &sources.iter().map(|&source| source.to_owned()).collect::<Vec<_>>(),
        )
    }
}

#[test]
fn only_stale_records_are_rewritten_and_the_rest_of_the_policy_is_kept() -> Result<()> {
    let repo = Monorepo::new()?;
    repo.write("src/a.rs", "a\n")?;
    repo.write("src/b.rs", "b\n")?;
    let current = repo.hash(&["src/b.rs"])?;
    let policy = format!(
        "# reviewed by hand\n[[records]]\npath = \"src/a.rs\"\nsource_sha256 = \"sha256:old\"\n\n\
         [[records]]\npath = \"src/b.rs\"\nsource_sha256 = \"{current}\"\n"
    );
    repo.write("governance/nested/policy.toml", &policy)?;
    let found = stale(&repo.workspace)?;
    assert_eq!(
        found,
        [Stale {
            policy: "governance/nested/policy.toml".into(),
            sources: vec!["src/a.rs".into()],
            recorded: "sha256:old".into(),
            actual: repo.hash(&["src/a.rs"])?,
        }]
    );
    assert_eq!(record(&repo.workspace)?, found);
    assert_eq!(
        repo.read("governance/nested/policy.toml")?,
        policy.replace("sha256:old", &repo.hash(&["src/a.rs"])?)
    );
    assert!(stale(&repo.workspace)?.is_empty());
    assert!(record(&repo.workspace)?.is_empty());
    Ok(())
}

#[test]
fn each_record_shape_reviews_the_sources_the_gates_select() -> Result<()> {
    let repo = Monorepo::new()?;
    for (file, text) in [("a.rs", "a"), ("b.rs", "b"), ("package.json", "{}"), ("lock.json", "[]")]
    {
        repo.write(file, text)?;
    }
    repo.write(
        "governance/policy.toml",
        "[[formats]]\nsources = [\"b.rs\", \"a.rs\"]\nsource_sha256 = \"x\"\n\
         [[scripts]]\npath = \"package.json\"\nlockfile = \"lock.json\"\nsource_sha256 = \"y\"\n",
    )?;
    record(&repo.workspace)?;
    let written = repo.read("governance/policy.toml")?;
    assert!(written.contains(&repo.hash(&["a.rs", "b.rs"])?));
    assert!(written.contains(&repo.hash(&["package.json", "lock.json"])?));
    Ok(())
}

#[test]
fn a_sibling_component_file_is_reviewed_outside_the_workspace() -> Result<()> {
    let repo = Monorepo::new()?;
    repo.write("a.rs", "a")?;
    let sibling = repo.workspace.with_file_name("Sugar");
    std::fs::create_dir_all(&sibling)
        .map_err(|error| Failure::external("create sibling", error))?;
    std::fs::write(sibling.join("schema.json"), "{}")
        .map_err(|error| Failure::external("write sibling", error))?;
    repo.write(
        "governance/policy.toml",
        "[[formats]]\nsources = [\"a.rs\", \"Sugar/schema.json\"]\nsource_sha256 = \"x\"\n",
    )?;
    let [found] = stale(&repo.workspace)?.try_into().map_err(|_| Failure::Policy("one".into()))?;
    let expected = fingerprint_resolved(&found.sources, |source| match source {
        "Sugar/schema.json" => resolve_sibling(&repo.workspace, source),
        _ => resolve(&repo.workspace, source),
    })?;
    assert_eq!(found.actual, expected);
    Ok(())
}

#[test]
fn an_unresolvable_source_refuses_and_rewrites_nothing() -> Result<()> {
    let repo = Monorepo::new()?;
    repo.write("a.rs", "a")?;
    let policy = "[[a]]\npath = \"a.rs\"\nsource_sha256 = \"x\"\n\
                  [[b]]\npath = \"missing.rs\"\nsource_sha256 = \"y\"\n";
    repo.write("governance/policy.toml", policy)?;
    assert!(record(&repo.workspace).is_err());
    assert_eq!(repo.read("governance/policy.toml")?, policy);
    for escaping in ["../outside.rs", "/absolute.rs", "folder\\file.rs"] {
        repo.write(
            "governance/policy.toml",
            &format!("path = '{escaping}'\nsource_sha256 = 'x'\n"),
        )?;
        assert!(record(&repo.workspace).is_err(), "{escaping}");
    }
    Ok(())
}

#[test]
fn tables_without_a_reviewed_fingerprint_are_left_alone() -> Result<()> {
    let repo = Monorepo::new()?;
    let policy = "[[records]]\npath = \"absent.rs\"\nkind = \"fixture\"\n\
                  [[owners]]\nsources = [\"absent.rs\"]\n";
    repo.write("governance/policy.toml", policy)?;
    assert!(record(&repo.workspace)?.is_empty());
    assert_eq!(repo.read("governance/policy.toml")?, policy);
    Ok(())
}

#[test]
fn records_sharing_a_placeholder_each_receive_their_own_fingerprint() -> Result<()> {
    let repo = Monorepo::new()?;
    repo.write("a.rs", "a")?;
    repo.write("b.rs", "b")?;
    repo.write(
        "governance/policy.toml",
        "[[records]]\npath = \"a.rs\"\nsource_sha256 = \"sha256:unreviewed\"\n\n\
         [[records]]\npath = \"b.rs\"\nsource_sha256 = \"sha256:unreviewed\"\n",
    )?;
    assert_eq!(record(&repo.workspace)?.len(), 2);
    assert_eq!(
        repo.read("governance/policy.toml")?,
        format!(
            "[[records]]\npath = \"a.rs\"\nsource_sha256 = \"{}\"\n\n\
             [[records]]\npath = \"b.rs\"\nsource_sha256 = \"{}\"\n",
            repo.hash(&["a.rs"])?,
            repo.hash(&["b.rs"])?
        )
    );
    Ok(())
}

#[test]
fn a_multiline_source_list_and_nested_tables_are_read_per_table() -> Result<()> {
    let repo = Monorepo::new()?;
    repo.write("a.rs", "a")?;
    repo.write("b.rs", "b")?;
    repo.write(
        "governance/policy.toml",
        "schema_version = 1\n\n[[boundaries]]\nsources = [\n  \"a.rs\",\n  \"b.rs\",\n]\n\
         source_sha256 = \"x\"\n\n[boundaries.detail]\nnote = \"kept\"\n",
    )?;
    record(&repo.workspace)?;
    let written = repo.read("governance/policy.toml")?;
    assert!(written.contains(&repo.hash(&["a.rs", "b.rs"])?));
    assert!(written.ends_with("[boundaries.detail]\nnote = \"kept\"\n"));
    Ok(())
}
