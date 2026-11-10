// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Preserve byte-exact control files before publishing a complete native-bound recovery manifest.
use super::{
    controls::{ControlFile, Controls, Kind},
    installation::InstallationData,
};
use crate::durable_storage::{
    check,
    files::{self, Directory},
    invalid, protection, record_publication, user_paths,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{fs::File, io, os::windows::fs::MetadataExt};
use sugarsubstitute_contracts::{
    execution::JobContext,
    installation::{Generation, InstallationId},
};

const MANIFEST: &str = "originals-v1.json";
const MANIFEST_BYTES: usize = 512 * 1024;
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Original {
    kind: Kind,
    source: String,
    native: (u32, u32, u32),
    leaf: String,
    bytes: u64,
    sha256: String,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Record {
    schema_version: u32,
    installation: InstallationId,
    generation: Generation,
    root: [u8; 24],
    directory: [u8; 24],
    #[serde(deserialize_with = "read_originals")]
    originals: Vec<Original>,
}

/// The first complete snapshot remains immutable when later valid user edits change startup bytes.
/// Source paths/native identities are historical provenance, never cleanup or launch authority.
pub(super) fn preserve(
    installation: &InstallationData,
    controls: &Controls,
    context: &JobContext,
) -> io::Result<()> {
    check(context)?;
    let mut key = Sha256::new();
    key.update(b"SugarSubstitute.original-launch-inputs.v1");
    for (kind, file) in controls.files() {
        let path =
            file.path.to_str().ok_or_else(|| invalid("Original control path is not Unicode"))?;
        for part in [kind.name().as_bytes(), path.as_bytes()] {
            key.update((part.len() as u64).to_le_bytes());
            key.update(part);
        }
    }
    let path = installation
        .root
        .path()
        .join("user")
        .join("migration-originals")
        .join(installation.identity.id.as_str())
        .join(format!("{:x}", key.finalize()));
    let directory = user_paths::create_parents(&installation.root, &path, context)?;
    protection::with(context, |owner| {
        let _writer = owner.exclusive_rendezvous(&directory.path().join("writer"), context)?;
        inventory(&directory, context)?;
        match directory.read(MANIFEST) {
            Ok(mut file) => {
                files::single_link(&file)?;
                owner.verify_file(&file)?;
                let bytes = files::read_admitted(&mut file, MANIFEST_BYTES, context)?;
                let record: Record = serde_json::from_slice(&bytes).map_err(io::Error::other)?;
                verify(&record, installation, controls)?;
                let _copies = verify_copies(&directory, &record, owner, context)?;
                return check(context);
            }
            Err(cause) if cause.kind() == io::ErrorKind::NotFound => {}
            Err(cause) => return Err(cause),
        }
        let mut originals = Vec::new();
        for (kind, input) in controls.files() {
            let digest = format!("{:x}", Sha256::digest(&input.bytes));
            let leaf = format!("{}-{digest}.{}", kind.name(), kind.extension());
            match directory.read(&leaf) {
                Ok(mut file) => {
                    files::single_link(&file)?;
                    owner.verify_file(&file)?;
                    if files::read_admitted(&mut file, kind.limit(), context)? != input.bytes {
                        return Err(invalid(
                            "Existing original recovery bytes differ; both are preserved",
                        ));
                    }
                }
                Err(cause) if cause.kind() == io::ErrorKind::NotFound => {
                    record_publication::publish(
                        &directory,
                        &leaf,
                        &input.bytes,
                        kind.limit(),
                        context,
                    )?;
                }
                Err(cause) => return Err(cause),
            }
            originals.push(original(kind, input, leaf, digest)?);
        }
        let record = Record {
            schema_version: 1,
            installation: installation.snapshot.installation().clone(),
            generation: installation.identity.clone(),
            root: installation.root.native_identity()?,
            directory: installation.generation.native_identity()?,
            originals,
        };
        verify(&record, installation, controls)?;
        let _copies = verify_copies(&directory, &record, owner, context)?;
        let bytes = serde_json::to_vec(&record).map_err(io::Error::other)?;
        record_publication::publish(&directory, MANIFEST, &bytes, MANIFEST_BYTES, context)?;
        check(context)
    })
}
fn original(kind: Kind, file: &ControlFile, leaf: String, sha256: String) -> io::Result<Original> {
    Ok(Original {
        kind,
        source: file
            .path
            .to_str()
            .ok_or_else(|| invalid("Original path is not Unicode"))?
            .to_owned(),
        native: files::file_identity(&file.file)?,
        leaf,
        bytes: file.bytes.len() as u64,
        sha256,
    })
}
fn verify(record: &Record, installation: &InstallationData, controls: &Controls) -> io::Result<()> {
    if record.schema_version != 1
        || &record.installation != installation.snapshot.installation()
        || record.generation != installation.identity
        || record.root != installation.root.native_identity()?
        || record.directory != installation.generation.native_identity()?
        || record.originals.len() != controls.files().count()
    {
        return Err(invalid("Original launch recovery association is incompatible"));
    }
    for (kind, file) in controls.files() {
        let matches =
            record.originals.iter().filter(|original| original.kind == kind).collect::<Vec<_>>();
        let [original] = matches.as_slice() else {
            return Err(invalid("Original launch recovery roles disagree"));
        };
        if file.path.to_str() != Some(original.source.as_str())
            || original.source.len() > 128 * 1024
            || original.bytes > kind.limit() as u64
            || !digest(&original.sha256)
            || original.leaf != format!("{}-{}.{}", kind.name(), original.sha256, kind.extension())
        {
            return Err(invalid("Original launch recovery source is incompatible"));
        }
    }
    Ok(())
}
fn verify_copies(
    directory: &Directory,
    record: &Record,
    owner: &protection::Protection,
    context: &JobContext,
) -> io::Result<Vec<File>> {
    let mut pins: Vec<File> = Vec::new();
    for original in &record.originals {
        let mut file = directory.read(&original.leaf)?;
        files::single_link(&file)?;
        owner.verify_file(&file)?;
        let bytes = files::read_admitted(&mut file, original.kind.limit(), context)?;
        if bytes.len() as u64 != original.bytes
            || format!("{:x}", Sha256::digest(&bytes)) != original.sha256
        {
            return Err(invalid("Original launch recovery bytes changed"));
        }
        pins.push(file);
    }
    check(context)?;
    Ok(pins)
}
/// Bound record allocation while decoding, before post-decode role validation.
fn read_originals<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<Vec<Original>, D::Error> {
    struct Originals;
    impl<'de> serde::de::Visitor<'de> for Originals {
        type Value = Vec<Original>;
        fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            formatter.write_str("at most three original launch controls")
        }
        fn visit_seq<A: serde::de::SeqAccess<'de>>(
            self,
            mut sequence: A,
        ) -> Result<Self::Value, A::Error> {
            let mut originals = Vec::new();
            originals.try_reserve_exact(3).map_err(serde::de::Error::custom)?;
            for _ in 0..3 {
                let Some(original) = sequence.next_element()? else {
                    return Ok(originals);
                };
                originals.push(original);
            }
            if sequence.next_element::<serde::de::IgnoredAny>()?.is_some() {
                return Err(serde::de::Error::custom(
                    "Original launch control inventory exceeds admission",
                ));
            }
            Ok(originals)
        }
    }
    deserializer.deserialize_seq(Originals)
}
fn digest(value: &str) -> bool {
    value.len() == 64
        && value.bytes().all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}
fn inventory(directory: &Directory, context: &JobContext) -> io::Result<()> {
    let mut visited = 0;
    for entry in directory.entries(context)? {
        let entry = entry?;
        visited += 1;
        if visited > 1024 {
            return Err(invalid("Original recovery inventory exceeds admission"));
        }
        let name = entry
            .file_name()
            .into_string()
            .map_err(|_| invalid("Original recovery name is not Unicode"))?;
        let metadata = entry.current_metadata()?;
        let copy = [Kind::Descriptor, Kind::Startup, Kind::Ready].iter().any(|kind| {
            name.strip_prefix(&format!("{}-", kind.name()))
                .and_then(|name| name.strip_suffix(&format!(".{}", kind.extension())))
                .is_some_and(digest)
        });
        if !metadata.is_file()
            || metadata.file_attributes() & 0x400 != 0
            || !(name == MANIFEST
                || name == "writer"
                || copy
                || (name.starts_with(".import-record-")
                    && name.len() <= 128
                    && files::flat_leaf(&name).is_ok()))
        {
            return Err(invalid("Unknown original recovery entries are preserved"));
        }
    }
    Ok(())
}
