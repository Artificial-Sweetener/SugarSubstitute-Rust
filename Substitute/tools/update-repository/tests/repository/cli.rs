// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! The command line the update repository workflow runs, end to end on throwaway keys, with the
//! application's verifier as the judge of every result.

use super::support::*;
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
};
use sugarsubstitute_update_repository::storage;

const TOOL: &str = env!("CARGO_BIN_EXE_sugarsubstitute-update-repository");

struct Folder {
    _scratch: tempfile::TempDir,
    root: PathBuf,
}
impl Folder {
    fn new() -> Result<Self, std::io::Error> {
        let scratch = tempfile::tempdir()?;
        let root = scratch.path().to_path_buf();
        for name in ["keys", "repository", "targets"] {
            fs::create_dir(root.join(name))?;
        }
        Ok(Self { _scratch: scratch, root })
    }
    fn path(&self, name: &str) -> PathBuf {
        self.root.join(name)
    }
    /// Run the tool; the timestamp key is passed through the environment, as CI passes secrets.
    fn run(&self, arguments: &[&str]) -> Result<Output, std::io::Error> {
        let seed = fs::read_to_string(self.path("keys/timestamp")).unwrap_or_default();
        Command::new(TOOL).args(arguments).env("UPDATE_TIMESTAMP_KEY", seed.trim()).output()
    }
}

fn text(output: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

fn read(
    folder: &Path,
) -> Result<sugarsubstitute_update_repository::Files, Box<dyn std::error::Error>> {
    Ok(storage::read_files(folder)?)
}

#[test]
fn the_workflow_commands_produce_a_repository_installations_accept() -> TestResult {
    let folder = Folder::new()?;
    for role in ["root", "targets", "snapshot", "timestamp"] {
        let output = folder.run(&[
            "keygen",
            "--output",
            &folder.path(&format!("keys/{role}")).to_string_lossy(),
        ])?;
        assert!(output.status.success(), "{}", text(&output));
    }
    let refused =
        folder.run(&["keygen", "--output", &folder.path("keys/root").to_string_lossy()])?;
    assert!(!refused.status.success(), "keygen overwrote a key");

    let repository = folder.path("repository");
    let repository = repository.to_string_lossy();
    let key = |role: &str| format!("file:{}", folder.path(&format!("keys/{role}")).display());
    let online = [
        "--targets-key".to_owned(),
        key("targets"),
        "--snapshot-key".to_owned(),
        key("snapshot"),
        "--timestamp-key".to_owned(),
        "env:UPDATE_TIMESTAMP_KEY".to_owned(),
    ];
    let start = START.to_string();
    let mut init = vec!["init", "--repository", &repository, "--now", &start];
    let root_key = key("root");
    init.extend(["--root-key", &root_key]);
    init.extend(online.iter().map(String::as_str));
    let output = folder.run(&init)?;
    assert!(output.status.success(), "{}", text(&output));
    let trusted = folder.path("trusted.root.json");
    fs::copy(folder.path("repository/1.root.json"), &trusted)?;
    let trusted = trusted.to_string_lossy();

    let release = Release::new(1, "0.2.0")?;
    let package = folder.path(&format!("targets/{}", release.package_name));
    let descriptor = folder.path(&format!("targets/{}", request().target_name()));
    fs::write(&package, &release.package)?;
    fs::write(&descriptor, &release.descriptor)?;
    let manifest = folder.path("written.txt");
    let manifest_text = manifest.to_string_lossy();
    let released = (START + DAY).to_string();
    let mut command = vec![
        "release",
        "--repository",
        &repository,
        "--trusted-root",
        &trusted,
        "--now",
        &released,
    ];
    command.extend(online.iter().map(String::as_str));
    let (package_text, descriptor_text) = (package.to_string_lossy(), descriptor.to_string_lossy());
    command.extend([
        "--target",
        &package_text,
        "--target",
        &descriptor_text,
        "--written",
        &manifest_text,
    ]);
    let output = folder.run(&command)?;
    assert!(output.status.success(), "{}", text(&output));
    assert_eq!(
        fs::read_to_string(&manifest)?,
        format!("2.snapshot.json\n2.targets.json\n{}\ntimestamp.json\n", request().target_name())
    );
    assert!(text(&output).contains(&format!("offer {}", request().target_name())));
    assert!(text(&output).contains(&format!("package {}", release.package_name)));

    let before = folder.path("before");
    fs::create_dir(&before)?;
    for (name, bytes) in read(&folder.path("repository"))? {
        fs::write(before.join(name), bytes)?;
    }
    let refreshed = (START + 2 * DAY).to_string();
    let output = folder.run(&[
        "refresh-timestamp",
        "--repository",
        &repository,
        "--trusted-root",
        &trusted,
        "--now",
        &refreshed,
        "--timestamp-key",
        "env:UPDATE_TIMESTAMP_KEY",
    ])?;
    assert!(output.status.success(), "{}", text(&output));
    let expires = START + 2 * DAY + 14 * DAY;
    assert!(text(&output).contains(&format!("timestamp 3 {expires}")), "{}", text(&output));
    let output = folder.run(&[
        "verify",
        "--repository",
        &repository,
        "--trusted-root",
        &trusted,
        "--now",
        &refreshed,
        "--previous",
        &before.to_string_lossy(),
        "--files",
        &folder.path("targets").to_string_lossy(),
    ])?;
    assert!(output.status.success(), "{}", text(&output));
    assert!(text(&output).contains(&format!("package {}", release.package_name)));

    let packages = BTreeMap::from([(release.package_name.clone(), release.package.clone())]);
    let files = read(&folder.path("repository"))?;
    let shipped = fs::read(folder.path("trusted.root.json"))?;
    assert_eq!(
        Installation::new(&shipped)?.check(&files, &packages, START + 2 * DAY + 1)?,
        "0.2.0"
    );
    Ok(())
}

#[test]
fn a_published_version_is_never_rewritten_and_the_timestamp_stays_put() -> TestResult {
    let folder = Folder::new()?;
    for role in ["root", "targets", "snapshot", "timestamp"] {
        assert!(
            folder
                .run(&[
                    "keygen",
                    "--output",
                    &folder.path(&format!("keys/{role}")).to_string_lossy()
                ])?
                .status
                .success()
        );
    }
    let repository = folder.path("repository").to_string_lossy().into_owned();
    let key = |role: &str| format!("file:{}", folder.path(&format!("keys/{role}")).display());
    let start = START.to_string();
    let output = folder.run(&[
        "init",
        "--repository",
        &repository,
        "--now",
        &start,
        "--root-key",
        &key("root"),
        "--targets-key",
        &key("targets"),
        "--snapshot-key",
        &key("snapshot"),
        "--timestamp-key",
        &key("timestamp"),
    ])?;
    assert!(output.status.success(), "{}", text(&output));
    let trusted = folder.path("repository/1.root.json").to_string_lossy().into_owned();
    fs::write(folder.path("repository/2.targets.json"), b"squatting")?;
    let timestamp = fs::read(folder.path("repository/timestamp.json"))?;
    let later = (START + DAY).to_string();
    let output = folder.run(&[
        "resign",
        "--repository",
        &repository,
        "--trusted-root",
        &trusted,
        "--now",
        &later,
        "--targets-key",
        &key("targets"),
        "--snapshot-key",
        &key("snapshot"),
        "--timestamp-key",
        &key("timestamp"),
    ])?;
    assert!(!output.status.success());
    assert!(
        text(&output).contains("2.targets.json is already published and is never rewritten"),
        "{}",
        text(&output)
    );
    assert_eq!(fs::read(folder.path("repository/timestamp.json"))?, timestamp);
    assert!(!folder.path("repository/2.snapshot.json").exists());
    let output =
        folder.run(&["resign", "--repository", &repository, "--now", &later, "--bogus", "1"])?;
    assert!(!output.status.success());
    let output = folder.run(&[
        "refresh-timestamp",
        "--repository",
        &repository,
        "--trusted-root",
        &trusted,
        "--now",
        &later,
        "--timestamp-key",
        "env:UNSET_UPDATE_KEY_FOR_TEST",
    ])?;
    assert!(text(&output).contains("UNSET_UPDATE_KEY_FOR_TEST is not set"), "{}", text(&output));
    Ok(())
}
