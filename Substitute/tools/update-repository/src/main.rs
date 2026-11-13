// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Command line for signing and verifying the SugarSubstitute update repository.
//!
//! ```text
//! keygen            --output FILE
//! init              --repository DIR --now SECONDS --root-key KEY ONLINE
//! refresh-timestamp --repository DIR --trusted-root FILE --now SECONDS --timestamp-key KEY
//! resign            --repository DIR --trusted-root FILE --now SECONDS ONLINE
//! release           --repository DIR --trusted-root FILE --now SECONDS ONLINE --target FILE...
//! rotate-root       --repository DIR --trusted-root FILE --now SECONDS
//!                   --current-root-key KEY --root-key KEY ONLINE
//! verify            --repository DIR --trusted-root FILE --now SECONDS
//!                   [--previous DIR] [--files DIR]
//! ```
//!
//! Signing commands also take `--written FILE`, which receives the names they wrote, one per
//! line, in the order to upload them: versioned metadata, channel descriptors, the timestamp.
//! `release` reads the packages and channel descriptors it signs from its `--target` files.
//!
//! `ONLINE` is `--targets-key KEY --snapshot-key KEY --timestamp-key KEY`; a `KEY` is `env:NAME`
//! or `file:PATH` holding a hexadecimal Ed25519 seed. `--now` is Unix UTC seconds. Every signing
//! command authenticates the published state first and verifies its result with the
//! application's verifier before writing anything. Developer diagnostics go to standard error.
use std::{collections::BTreeMap, path::PathBuf, process::ExitCode};
use sugarsubstitute_update_repository::{
    Change, Error, OnlineKeys, Published, Report, initialize,
    keys::SigningKey,
    prepare,
    storage::{self, PayloadFiles},
    verify,
};

fn main() -> ExitCode {
    match run(std::env::args().skip(1).collect()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(cause) => {
            eprintln!("error: {cause}");
            ExitCode::FAILURE
        }
    }
}

/// Parsed `--name value` options; `--target` may repeat.
struct Options {
    values: BTreeMap<String, Vec<String>>,
}
impl Options {
    fn parse(arguments: &[String]) -> Result<Self, Error> {
        let mut values: BTreeMap<String, Vec<String>> = BTreeMap::new();
        let mut rest = arguments.iter();
        while let Some(name) = rest.next() {
            let name = name
                .strip_prefix("--")
                .ok_or_else(|| Error::Invalid(format!("Unexpected argument {name}")))?;
            let value =
                rest.next().ok_or_else(|| Error::Invalid(format!("--{name} needs a value")))?;
            let entry = values.entry(name.to_owned()).or_default();
            if !entry.is_empty() && name != "target" {
                return Err(Error::Invalid(format!("--{name} is given twice")));
            }
            entry.push(value.clone());
        }
        Ok(Self { values })
    }
    fn optional(&mut self, name: &str) -> Option<String> {
        self.values.remove(name).and_then(|mut values| values.pop())
    }
    fn required(&mut self, name: &str) -> Result<String, Error> {
        self.optional(name).ok_or_else(|| Error::Invalid(format!("--{name} is required")))
    }
    fn all(&mut self, name: &str) -> Vec<String> {
        self.values.remove(name).unwrap_or_default()
    }
    fn now(&mut self) -> Result<i64, Error> {
        self.required("now")?
            .parse()
            .map_err(|_| Error::Invalid("--now is Unix UTC seconds".into()))
    }
    fn key(&mut self, name: &str) -> Result<SigningKey, Error> {
        storage::read_key(&self.required(name)?)
    }
    fn finish(self) -> Result<(), Error> {
        match self.values.keys().next() {
            Some(name) => Err(Error::Invalid(format!("--{name} is not an option of this command"))),
            None => Ok(()),
        }
    }
}

/// The three online role keys.
struct Online {
    targets: SigningKey,
    snapshot: SigningKey,
    timestamp: SigningKey,
}
impl Online {
    fn read(options: &mut Options) -> Result<Self, Error> {
        Ok(Self {
            targets: options.key("targets-key")?,
            snapshot: options.key("snapshot-key")?,
            timestamp: options.key("timestamp-key")?,
        })
    }
    fn keys(&self) -> OnlineKeys<'_> {
        OnlineKeys { targets: &self.targets, snapshot: &self.snapshot, timestamp: &self.timestamp }
    }
}

fn run(arguments: Vec<String>) -> Result<(), Error> {
    let (command, rest) =
        arguments.split_first().ok_or_else(|| Error::Invalid("No command given".into()))?;
    let mut options = Options::parse(rest)?;
    if command == "keygen" {
        let output = PathBuf::from(options.required("output")?);
        options.finish()?;
        let (key, seed) = SigningKey::generate()?;
        storage::write_key(&output, &seed)?;
        println!("{}", key.public().key_id());
        return Ok(());
    }
    let folder = PathBuf::from(options.required("repository")?);
    let now = options.now()?;
    if command == "init" {
        let root = options.key("root-key")?;
        let online = Online::read(&mut options)?;
        let written = options.optional("written").map(PathBuf::from);
        options.finish()?;
        let published = initialize(&root, &online.keys(), now)?;
        let trusted = published.files.get("1.root.json").cloned().unwrap_or_default();
        let report = verify(&trusted, None, &published.files, &PayloadFiles::none(), now)?;
        return publish(&folder, &published, &report, written);
    }
    let trusted_root = storage::read_metadata(&PathBuf::from(options.required("trusted-root")?))?;
    let current = storage::read_files(&folder)?;
    if command == "verify" {
        let previous = options.optional("previous").map(PathBuf::from);
        let payloads = options.optional("files").map(PathBuf::from);
        options.finish()?;
        let payloads = match payloads {
            Some(folder) => PayloadFiles::folder(&folder)?,
            None => PayloadFiles::none(),
        };
        let previous = previous.map(|path| storage::read_files(&path)).transpose()?;
        let report = verify(&trusted_root, previous.as_ref(), &current, &payloads, now)?;
        print_report(&report);
        return Ok(());
    }
    let written = options.optional("written").map(PathBuf::from);
    let mut payloads = PayloadFiles::none();
    let online;
    let current_root;
    let next_root;
    let timestamp;
    let change = match command.as_str() {
        "refresh-timestamp" => {
            timestamp = options.key("timestamp-key")?;
            Change::RefreshTimestamp { timestamp: &timestamp }
        }
        "resign" => {
            online = Online::read(&mut options)?;
            Change::Resign(online.keys())
        }
        "release" => {
            online = Online::read(&mut options)?;
            let paths: Vec<PathBuf> =
                options.all("target").into_iter().map(PathBuf::from).collect();
            payloads = PayloadFiles::files(&paths)?;
            let targets = paths
                .iter()
                .map(|path| storage::describe_target(path))
                .collect::<Result<Vec<_>, _>>()?;
            if targets.is_empty() {
                return Err(Error::Invalid("A release needs at least one --target".into()));
            }
            Change::Release { keys: online.keys(), targets }
        }
        "rotate-root" => {
            current_root = options.key("current-root-key")?;
            next_root = options.key("root-key")?;
            online = Online::read(&mut options)?;
            Change::RotateRoot {
                current_root: &current_root,
                next_root: &next_root,
                keys: online.keys(),
            }
        }
        other => return Err(Error::Invalid(format!("Unknown command {other}"))),
    };
    options.finish()?;
    let (published, report) = prepare(&current, &trusted_root, change, &payloads, now)?;
    publish(&folder, &published, &report, written)
}

/// Write the verified files, then say what was written and what installations will accept.
fn publish(
    folder: &std::path::Path,
    published: &Published,
    report: &Report,
    manifest: Option<PathBuf>,
) -> Result<(), Error> {
    let written = storage::write(folder, published)?;
    for name in &written {
        eprintln!("wrote {name}");
    }
    if let Some(manifest) = manifest {
        storage::write_manifest(&manifest, &written)?;
    }
    print_report(report);
    Ok(())
}

/// One `role version expires` line per role, then the accepted offers and packages.
fn print_report(report: &Report) {
    for (role, (version, expires)) in [
        ("root", report.root),
        ("timestamp", report.timestamp),
        ("snapshot", report.snapshot),
        ("targets", report.targets),
    ] {
        println!("{role} {version} {expires}");
    }
    for offer in &report.offers {
        println!("offer {offer}");
    }
    for package in &report.packages {
        println!("package {package}");
    }
}
